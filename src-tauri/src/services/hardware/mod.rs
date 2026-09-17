//! Best-effort hardware detection used to pick practical local models.
//! Every probe is optional: CPU-only machines without a GPU are fully supported.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub vram_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub os: String,
    pub cpu_name: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub gpus: Vec<GpuInfo>,
}

impl HardwareInfo {
    /// Largest dedicated VRAM of any detected GPU (0 when none).
    pub fn best_vram(&self) -> u64 {
        self.gpus.iter().map(|g| g.vram_bytes).max().unwrap_or(0)
    }

    pub fn has_nvidia(&self) -> bool {
        self.gpus.iter().any(|g| g.vendor == "nvidia")
    }

    /// Threads to give an inference engine: leave headroom for the UI and LM Studio.
    pub fn inference_threads(&self) -> i32 {
        let cores = self.physical_cores.max(1);
        (cores.saturating_sub(1)).clamp(1, 8) as i32
    }
}

pub fn detect() -> HardwareInfo {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_default();
    HardwareInfo {
        os: sysinfo::System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string()),
        cpu_name,
        physical_cores: sysinfo::System::physical_core_count().unwrap_or(1),
        logical_cores: sys.cpus().len().max(1),
        total_ram_bytes: sys.total_memory(),
        available_ram_bytes: sys.available_memory(),
        gpus: detect_gpus(),
    }
}

fn vendor_name(id: u32) -> &'static str {
    match id {
        0x10DE => "nvidia",
        0x1002 | 0x1022 => "amd",
        0x8086 | 0x8087 => "intel",
        0x1414 => "microsoft",
        0x5143 => "qualcomm",
        _ => "other",
    }
}

#[cfg(windows)]
fn detect_gpus() -> Vec<GpuInfo> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
    let mut out = Vec::new();
    // SAFETY: plain COM calls; every returned interface is reference counted by the bindings.
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else {
            return out;
        };
        let mut i = 0;
        while let Ok(adapter) = factory.EnumAdapters1(i) {
            i += 1;
            let Ok(desc) = adapter.GetDesc1() else { continue };
            if desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
                continue;
            }
            let vendor = vendor_name(desc.VendorId);
            if vendor == "microsoft" {
                continue; // Basic Render Driver
            }
            let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
            out.push(GpuInfo {
                name: String::from_utf16_lossy(&desc.Description[..len]).trim().to_string(),
                vendor: vendor.to_string(),
                vram_bytes: desc.DedicatedVideoMemory as u64,
            });
        }
    }
    out
}

#[cfg(not(windows))]
fn detect_gpus() -> Vec<GpuInfo> {
    // nvidia-smi is the most portable probe on Linux; macOS unified memory is
    // treated as CPU RAM by the model selector.
    let Ok(output) = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|l| {
            let (name, mem) = l.rsplit_once(',')?;
            Some(GpuInfo {
                name: name.trim().to_string(),
                vendor: "nvidia".into(),
                vram_bytes: mem.trim().parse::<u64>().ok()? * 1024 * 1024,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_does_not_panic_and_reports_ram() {
        let hw = detect();
        assert!(hw.total_ram_bytes > 0);
        assert!(hw.logical_cores >= 1);
        assert!(hw.inference_threads() >= 1 && hw.inference_threads() <= 8);
    }

    #[test]
    fn best_vram_empty() {
        assert_eq!(HardwareInfo::default().best_vram(), 0);
    }
}
