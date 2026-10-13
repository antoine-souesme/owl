// App de la barre des menus : une icône avec le compteur, un panneau au
// clic. Pas d'icône dans le Dock (`LSUIElement` dans `Info.plist`).
import AppKit
import OwlBarCore
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let state = BarState(
        runner: ProcessRunner(),
        notifier: SystemNotifier(),
        openURL: { NSWorkspace.shared.open($0) }
    )
    private var loop: Task<Void, Never>?
    private var animation: Task<Void, Never>?

    func applicationDidFinishLaunching(_ notification: Notification) {
        SystemNotifier.requestPermission()
        LaunchAtLogin.registerOnFirstLaunch()
        // L'horloge de l'animation, au rythme de la TUI : 100 ms.
        animation = Task { [state] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 100_000_000)
                state.tick()
            }
        }
        loop = Task { [state] in
            await state.refresh()
            // Un intervalle à zéro arrête la boucle : seul le bouton du
            // panneau rafraîchit alors.
            while let delay = state.nextRefreshDelay {
                try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
                await state.refresh()
            }
        }
    }
}

@main
struct OwlBarApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate

    var body: some Scene {
        MenuBarExtra {
            PanelView(state: delegate.state)
        } label: {
            MenuBarLabel(state: delegate.state)
        }
        .menuBarExtraStyle(.window)
    }
}

struct MenuBarLabel: View {
    @ObservedObject var state: BarState

    var body: some View {
        Text(state.title)
    }
}
