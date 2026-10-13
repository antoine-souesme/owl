import OwlBarCore
import SwiftUI

struct DetailView: View {
    @ObservedObject var state: BarState

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Button {
                    state.back()
                } label: {
                    Label("Back", systemImage: "chevron.left")
                }
                .buttonStyle(.borderless)
                Spacer()
                if let url = state.openPullRequest.flatMap({ URL(string: $0.url) }) {
                    Link("Open on GitHub", destination: url)
                }
            }
            .padding(.horizontal, 10)
            .padding(.top, 10)

            // Le titre reste en place, hors de la zone qui défile.
            Text(state.openPullRequest?.title ?? "")
                .font(.headline)
                .lineLimit(2)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(10)

            Divider()

            switch state.detail {
            case .loading:
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            case .failed(let message):
                Text(message)
                    .textSelection(.enabled)
                    .padding()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            case .loaded(let detail):
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        ForEach(state.detailHeaderLines, id: \.self) { line in
                            Text(line).foregroundStyle(.secondary)
                        }
                        if !detail.body.isEmpty {
                            Text(detail.body).textSelection(.enabled)
                        }
                        ForEach(state.detailSectionList, id: \.title) { section in
                            if !section.lines.isEmpty {
                                Text(section.title).font(.headline).foregroundStyle(Palette.sectionTitle.color)
                                ForEach(Array(section.lines.enumerated()), id: \.offset) { _, line in
                                    LineView(line: line)
                                }
                            }
                        }
                    }
                    .padding(10)
                }
            }

            Divider()
            MergeBar(state: state).padding(10)
        }
    }
}

struct LineView: View {
    let line: DetailLine

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 6) {
                if let glyph = line.glyph { GlyphView(glyph: glyph) }
                Text(line.text)
            }
            if let note = line.note {
                Text(note).foregroundStyle(.secondary).textSelection(.enabled)
            }
        }
    }
}

struct MergeBar: View {
    @ObservedObject var state: BarState

    var body: some View {
        HStack {
            if state.mergeOptions.choices.isEmpty {
                Text(noMergeMethodMessage).foregroundStyle(.secondary)
            } else {
                Button("Merge") { state.startMerge() }
            }
            Spacer()
        }
    }
}
