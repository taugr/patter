//! macOS notification actions belong to the main application, not the capture helper.
use crate::store::Result;
use block2::{DynBlock, RcBlock};
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{Bool, ProtocolObject},
    AllocAnyThread,
};
use objc2_foundation::{NSArray, NSBundle, NSError, NSObject, NSObjectProtocol, NSSet, NSString};
use objc2_user_notifications::*;
use std::sync::OnceLock;
use tauri::Manager;

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
thread_local! { static DELEGATE: std::cell::RefCell<Option<Retained<ReminderDelegate>>> = const { std::cell::RefCell::new(None) }; }

define_class!(
    #[unsafe(super = NSObject)]
    struct ReminderDelegate;
    unsafe impl NSObjectProtocol for ReminderDelegate {}
    unsafe impl UNUserNotificationCenterDelegate for ReminderDelegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn present(
            &self,
            _center: &UNUserNotificationCenter,
            notification: &UNNotification,
            completion: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            let mut options =
                UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::List;
            if notification.request().content().sound().is_some() {
                options |= UNNotificationPresentationOptions::Sound;
            }
            completion.call((options,));
        }
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn response(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion: &DynBlock<dyn Fn()>,
        ) {
            if let Some(app) = APP.get() {
                let id = response.notification().request().identifier().to_string();
                crate::reminders::activate(
                    app,
                    &id,
                    response.actionIdentifier().to_string() == "record",
                );
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            completion.call(());
        }
    }
);
fn center() -> Result<Retained<UNUserNotificationCenter>> {
    // Apple's API raises an ObjC exception for unbundled executables.
    if !NSBundle::mainBundle()
        .bundlePath()
        .to_string()
        .ends_with(".app")
    {
        return Err("macOS notifications require the packaged Patter app.".into());
    }
    Ok(UNUserNotificationCenter::currentNotificationCenter())
}
pub fn initialize(app: &tauri::AppHandle) {
    let _ = APP.set(app.clone());
    if let Ok(center) = center() {
        let delegate: Retained<ReminderDelegate> =
            unsafe { msg_send![ReminderDelegate::alloc(), init] };
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        DELEGATE.with(|slot| *slot.borrow_mut() = Some(delegate));
        let action = UNNotificationAction::actionWithIdentifier_title_options(
            &NSString::from_str("record"),
            &NSString::from_str("Record…"),
            UNNotificationActionOptions::Foreground,
        );
        let category =
            UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
                &NSString::from_str("meeting"),
                &NSArray::from_retained_slice(&[action]),
                &NSArray::new(),
                UNNotificationCategoryOptions::empty(),
            );
        center.setNotificationCategories(&NSSet::from_retained_slice(&[category]));
        // Reminders are delivered by the running app, never left scheduled after quit.
        center.removeAllPendingNotificationRequests();
    }
}
#[tauri::command]
pub async fn request_reminder_permission() -> Result<bool> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = std::sync::Mutex::new(Some(tx));
    center()?.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &RcBlock::new(move |granted: Bool, error: *mut NSError| {
            let result = if error.is_null() {
                Ok(granted.as_bool())
            } else {
                Err(unsafe { &*error }.localizedDescription().to_string())
            };
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(result);
            }
        }),
    );
    rx.await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn notification_permission() -> Result<String> {
    permission().await
}
pub async fn permission() -> Result<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = std::sync::Mutex::new(Some(tx));
    center()?.getNotificationSettingsWithCompletionHandler(&RcBlock::new(
        move |settings: std::ptr::NonNull<UNNotificationSettings>| {
            let status = unsafe { settings.as_ref() }.authorizationStatus();
            let status = if status == UNAuthorizationStatus::Authorized {
                "allowed"
            } else if status == UNAuthorizationStatus::NotDetermined {
                "not requested"
            } else {
                "blocked"
            };
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(status.to_string());
            }
        },
    ));
    rx.await.map_err(|e| e.to_string())
}
pub async fn show(id: &str, title: &str, body: &str, sound: bool) -> Result<()> {
    let rx = {
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        content.setCategoryIdentifier(&NSString::from_str("meeting"));
        if sound {
            content.setSound(Some(&UNNotificationSound::defaultSound()));
        }
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(id),
            &content,
            None,
        );
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = std::sync::Mutex::new(Some(tx));
        center()?.addNotificationRequest_withCompletionHandler(
            &request,
            Some(&RcBlock::new(move |error: *mut NSError| {
                let result = if error.is_null() {
                    Ok(())
                } else {
                    Err(unsafe { &*error }.localizedDescription().to_string())
                };
                if let Some(tx) = tx.lock().unwrap().take() {
                    let _ = tx.send(result);
                }
            })),
        );
        // Retained ObjC values must not cross the await boundary.
        rx
    };
    rx.await.map_err(|e| e.to_string())?
}
pub fn remove(id: &str) {
    if let Ok(center) = center() {
        center.removeDeliveredNotificationsWithIdentifiers(&NSArray::from_retained_slice(&[
            NSString::from_str(id),
        ]));
    }
}
