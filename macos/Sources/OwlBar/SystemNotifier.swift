// Notifications du système. Un refus de l'utilisateur ou un lancement hors
// du paquet `.app` (par `swift run`) font simplement taire l'app.
import Foundation
import OwlBarCore
import UserNotifications

struct SystemNotifier: Notifying {
    static func requestPermission() {
        guard Bundle.main.bundleIdentifier != nil else { return }
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    func notify(title: String, body: String) {
        // Sans identifiant de paquet, `UNUserNotificationCenter` arrête le
        // programme : on ne tente rien.
        guard Bundle.main.bundleIdentifier != nil else { return }
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.sound = .default
        let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }
}
