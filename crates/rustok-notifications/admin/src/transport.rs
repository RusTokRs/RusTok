use crate::core::NotificationsAdminStatus;

/// Bootstrap transport facade.
///
/// No backend inbox endpoint exists in the foundation slice. Returning the
/// explicit foundation status avoids host-owned shadow state or invented data.
pub fn load_notifications_admin_status() -> NotificationsAdminStatus {
    NotificationsAdminStatus::foundation()
}
