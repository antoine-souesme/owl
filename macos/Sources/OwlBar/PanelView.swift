import OwlBarCore
import SwiftUI

struct PanelView: View {
    @ObservedObject var state: BarState

    var body: some View {
        Group {
            if state.openKey != nil {
                DetailView(state: state)
                    .transition(.move(edge: .trailing))
            } else {
                ListView(state: state)
                    .transition(.move(edge: .leading))
            }
        }
        .animation(.easeInOut(duration: 0.2), value: state.openKey)
        .frame(width: 480, height: 520)
    }
}

extension Tone {
    var color: Color {
        switch self {
        case .green: .green
        case .red: .red
        case .yellow: .yellow
        case .gray: .secondary
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
