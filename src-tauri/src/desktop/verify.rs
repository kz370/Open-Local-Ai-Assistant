//! Asks the signed-in Windows user to confirm their identity (Windows Hello
//! PIN, fingerprint, face or password) before a security-sensitive change.

use crate::errors::{AppError, AppResult};

/// Shows the Windows Hello prompt parented to `window` and returns `Ok(())`
/// only if the user verified successfully.
#[cfg(windows)]
pub async fn verify_user(window: &tauri::WebviewWindow, message: &str) -> AppResult<()> {
    let hwnd = window.hwnd().map_err(|e| AppError::Other(e.to_string()))?.0 as isize;
    let message = message.to_string();
    tokio::task::spawn_blocking(move || request(hwnd, &message))
        .await
        .map_err(|e| AppError::Other(e.to_string()))?
}

#[cfg(windows)]
fn request(hwnd: isize, message: &str) -> AppResult<()> {
    use windows::core::{factory, HSTRING};
    use windows::Security::Credentials::UI::{UserConsentVerificationResult as R, UserConsentVerifier, UserConsentVerifierAvailability as A};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::IUserConsentVerifierInterop;

    let win = |e: windows::core::Error| AppError::Other(format!("Windows Hello: {e}"));
    let availability = UserConsentVerifier::CheckAvailabilityAsync().and_then(|op| op.join()).map_err(win)?;
    if availability != A::Available {
        return Err(AppError::PermissionDenied(
            "Windows Hello is not set up on this device. Set up a PIN in Windows Settings > Accounts > Sign-in options.".into(),
        ));
    }
    let interop = factory::<UserConsentVerifier, IUserConsentVerifierInterop>().map_err(win)?;
    let op: windows_future::IAsyncOperation<R> =
        unsafe { interop.RequestVerificationForWindowAsync(HWND(hwnd as *mut _), &HSTRING::from(message)) }.map_err(win)?;
    match op.join().map_err(win)? {
        R::Verified => Ok(()),
        R::Canceled => Err(AppError::Cancelled),
        _ => Err(AppError::PermissionDenied("Windows verification failed.".into())),
    }
}

#[cfg(not(windows))]
pub async fn verify_user(_window: &tauri::WebviewWindow, _message: &str) -> AppResult<()> {
    Err(AppError::PermissionDenied("User verification is only supported on Windows.".into()))
}
