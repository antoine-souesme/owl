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
                    Image(systemName: "chevron.left")
                }
                .buttonStyle(.borderless)
                .keyboardShortcut(.leftArrow, modifiers: [])
                Text(state.openPullRequest?.title ?? "")
                    .font(.headline)
                    .lineLimit(2)
                Spacer()
                if let url = state.openPullRequest.flatMap({ URL(string: $0.url) }) {
                    Link("Open on GitHub", destination: url)
                }
            }
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
                                Text(section.title).font(.headline)
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
    @StateObject private var form = MergeForm()

    var body: some View {
        let options = state.mergeOptions
        let method = form.chosen ?? options.selected
        VStack(alignment: .leading, spacing: 6) {
            if options.choices.isEmpty {
                Text(noMergeMethodMessage).foregroundStyle(.secondary)
            } else if state.mergeStatus == .running {
                ProgressView().controlSize(.small)
            } else if state.confirmingMerge, let method {
                HStack {
                    if options.choices.count > 1 {
                        Picker("", selection: Binding(
                            get: { method },
                            set: { form.chosen = $0 }
                        )) {
                            ForEach(options.choices, id: \.self) { choice in
                                Text(methodLabel(choice)).tag(choice)
                            }
                        }
                        .labelsHidden()
                        .fixedSize()
                    }
                    Button(methodLabel(method)) {
                        Task { await state.merge(method) }
                    }
                    .keyboardShortcut(.defaultAction)
                    Button("Cancel") { state.cancelMerge() }
                        .keyboardShortcut(.cancelAction)
                }
            } else {
                Button("Merge") { state.requestMerge() }
                    .keyboardShortcut("m", modifiers: [])
            }
            if case .failed(let message) = state.mergeStatus {
                Text(message).foregroundStyle(.red).textSelection(.enabled)
            }
        }
        .onChange(of: state.openKey) { _, _ in
            form.chosen = nil
        }
    }
}

/// Méthode choisie dans la barre de fusion, propre à la vue. Un objet plutôt
/// que `@State` : sans Xcode, la macro de `@State` du SDK de macOS 27 n'a pas
/// son greffon.
final class MergeForm: ObservableObject {
    @Published var chosen: MergeMethod?
}
