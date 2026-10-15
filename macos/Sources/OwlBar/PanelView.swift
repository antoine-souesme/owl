import OwlBarCore
import SwiftUI

struct PanelView: View {
    @ObservedObject var state: BarState
    @StateObject private var login = LaunchAtLoginToggle()

    var body: some View {
        VStack(spacing: 0) {
            Picker("", selection: Binding(
                get: { state.tab },
                set: { tab in Task { await state.selectTab(tab) } }
            )) {
                Text("Pull requests").tag(PanelTab.pullRequests)
                Text("Servers").tag(PanelTab.servers)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding([.horizontal, .top], 10)

            switch state.tab {
            case .pullRequests:
                ListView(state: state)
                if let notice = state.notice {
                    Divider()
                    Text(notice)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(10)
                }
            case .servers:
                ServersView(servers: state.servers)
            }

            Divider()

            HStack {
                Toggle("Launch at login", isOn: $login.isOn)
                    .toggleStyle(.checkbox)
                Spacer()
                Text(versionLabel(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Button("Quit") { NSApplication.shared.terminate(nil) }
                    .keyboardShortcut("q")
            }
            .padding(10)
        }
        .overlay {
            if let render = state.mergeRendering {
                DialogOverlay(
                    render: render,
                    onConfirm: { Task { await state.confirmMerge() } },
                    // `cancelMerge` refuse déjà pendant l'envoi de la fusion.
                    onCancel: { state.cancelMerge() }
                )
            }
        }
        .background(PanelKeys(state: state))
        .onAppear { Task { await state.panelOpened() } }
        .frame(width: 480, height: 520)
    }
}

/// Le clavier du panneau. Des boutons invisibles portent les raccourcis ;
/// `BarState.press` décide de ce que fait chaque touche. Les lettres ne
/// valent que sur l'onglet des pull requests : sur Servers, elles doivent
/// pouvoir s'écrire dans le champ de recherche.
struct PanelKeys: View {
    @ObservedObject var state: BarState

    var body: some View {
        ZStack {
            key(.upArrow, .up)
            key(.downArrow, .down)
            key(.return, .enter)
            key(.escape, .escape)
            key("1", .tab1, modifiers: .command)
            key("2", .tab2, modifiers: .command)
            if state.tab == .pullRequests {
                key("m", .m)
                key("o", .o)
                key("r", .r)
            }
        }
        .opacity(0)
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    private func key(_ shortcut: KeyEquivalent, _ key: PanelKey, modifiers: EventModifiers = []) -> some View {
        Button("") { Task { await state.press(key) } }
            .keyboardShortcut(shortcut, modifiers: modifiers)
    }
}

/// Fenêtre au centre du panneau : fusion ou arrêt d'un serveur. Les lignes,
/// leurs couleurs et les boutons viennent du rendu.
struct DialogView: View {
    let render: MergeRender
    let onConfirm: () -> Void
    let onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(render.title)
                .font(.headline)
                .foregroundStyle(render.frameTone?.color ?? .primary)
            ForEach(Array(render.lines.enumerated()), id: \.offset) { _, line in
                line.reduce(Text("")) { text, segment in
                    text + Text(segment.text).foregroundColor(segment.tone?.color ?? .primary)
                }
                .fixedSize(horizontal: false, vertical: true)
            }
            if render.confirmLabel != nil || render.cancelLabel != nil {
                HStack {
                    Spacer()
                    if let cancel = render.cancelLabel {
                        Button(cancel, action: onCancel)
                    }
                    if let confirm = render.confirmLabel {
                        Button(confirm, action: onConfirm)
                    }
                }
                .padding(.top, 6)
            } else {
                ProgressView().controlSize(.small)
            }
        }
        .font(.system(.body, design: .monospaced))
        .padding(14)
        .frame(width: 420)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10))
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(render.frameTone?.color ?? Color.secondary.opacity(0.4), lineWidth: 1)
        )
    }
}

/// Fond assombri sous une fenêtre : un clic dessus annule, comme Échap.
struct DialogOverlay: View {
    let render: MergeRender
    let onConfirm: () -> Void
    let onCancel: () -> Void

    var body: some View {
        ZStack {
            Color.black.opacity(0.35)
                .contentShape(Rectangle())
                .onTapGesture(perform: onCancel)
            DialogView(render: render, onConfirm: onConfirm, onCancel: onCancel)
                // Un clic dans la fenêtre, hors des boutons, ne doit pas
                // traverser jusqu'au fond qui l'annule.
                .contentShape(Rectangle())
                .onTapGesture {}
        }
    }
}

extension Tone {
    var color: Color {
        switch self {
        case .green: .green
        case .red: .red
        case .yellow: .yellow
        case .gray: .secondary
        case .cyan: .cyan
        case .blue: .blue
        }
    }
}

struct GlyphView: View {
    let glyph: Glyph

    var body: some View {
        Text(glyph.symbol)
            .foregroundStyle(glyph.tone.color)
            .frame(width: 14)
    }
}
