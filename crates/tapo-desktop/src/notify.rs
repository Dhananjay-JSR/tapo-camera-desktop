//! Native desktop notifications (macOS banner / Windows toast / Linux dbus) for camera alerts.

/// Post one OS notification. Failures are logged, never fatal (dbus/notification daemon may be absent).
pub fn show(title: &str, body: &str) {
    if let Err(e) = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .show()
    {
        crate::logbuf::log(format!("notify: {e}"));
    }
}
