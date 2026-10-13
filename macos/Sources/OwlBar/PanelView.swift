import OwlBarCore
import SwiftUI

struct PanelView: View {
    @ObservedObject var state: BarState

    var body: some View {
        VStack(spacing: 0) {
            ListView(state: state)
            if let notice = state.notice {
                Divider()
                Text(notice)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(10)
            }
        }
        .overlay {
            if let render = state.mergeRendering {
                ZStack {
                    Color.black.opacity(0.35)
                    MergeDialogView(state: state, render: render)
                }
            }
        }
        .background(PanelKeys(state: state))
        .frame(width: 480, height: 520)
    }
}

/// Le clavier du panneau. Des boutons invisibles portent les raccourcis ;
/// `BarState.press` décide de ce que fait chaque touche, comme dans la TUI.
struct PanelKeys: View {
    @ObservedObject var state: BarState

    var body: some View {
        ZStack {
            key(.upArrow, .up)
            key(.downArrow, .down)
            key(.return, .enter)
            key(.escape, .escape)
            key("m", .m)
            key("o", .o)
            key("r", .r)
        }
        .opacity(0)
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    private func key(_ shortcut: KeyEquivalent, _ key: PanelKey) -> some View {
        Button("") { Task { await state.press(key) } }
            .keyboardShortcut(shortcut, modifiers: [])
    }
}

/// Fenêtre de fusion, au centre du panneau. Les lignes, leurs couleurs et
/// les boutons viennent de `mergeRender`.
struct MergeDialogView: View {
    @ObservedObject var state: BarState
    let render: MergeRender

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
                        Button(cancel) { state.cancelMerge() }
                    }
                    if let confirm = render.confirmLabel {
                        Button(confirm) { Task { await state.confirmMerge() } }
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
