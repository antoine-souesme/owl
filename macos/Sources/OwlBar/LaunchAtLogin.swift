// Lancement à l'ouverture de session. Inscrit une seule fois d'office, au
// premier lancement ; ensuite, seule la case du panneau décide.
import Combine
import Foundation
import ServiceManagement

enum LaunchAtLogin {
    private static let registeredKey = "launchAtLoginRegisteredOnce"

    static var isEnabled: Bool {
        SMAppService.mainApp.status == .enabled
    }

    static func registerOnFirstLaunch() {
        let defaults = UserDefaults.standard
        guard Bundle.main.bundleIdentifier != nil, !defaults.bool(forKey: registeredKey) else { return }
        try? SMAppService.mainApp.register()
        defaults.set(true, forKey: registeredKey)
    }

    static func set(_ enabled: Bool) {
        if enabled {
            try? SMAppService.mainApp.register()
        } else {
            try? SMAppService.mainApp.unregister()
        }
    }
}

/// État de la case « Launch at login ». Un objet plutôt que `@State` : sans
/// Xcode, la macro de `@State` du SDK de macOS 27 n'a pas son greffon.
final class LaunchAtLoginToggle: ObservableObject {
    @Published var isOn = LaunchAtLogin.isEnabled {
        didSet { LaunchAtLogin.set(isOn) }
    }
}
