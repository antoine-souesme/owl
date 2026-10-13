import OwlBarCore
import SwiftUI

struct ListView: View {
    @ObservedObject var state: BarState
    @StateObject private var login = LaunchAtLoginToggle()

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text(state.refreshLabel)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Button {
                    Task { await state.refresh() }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.borderless)
                .disabled(state.isRefreshing)
                .keyboardShortcut("r")
            }
            .padding(10)
            .background(KeyboardShortcuts(state: state))

            Divider()

            if let message = state.errorMessage {
                Text(message)
                    .textSelection(.enabled)
                    .multilineTextAlignment(.center)
                    .padding()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollViewReader { scroller in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(state.rows) { row in
                            Button {
                                Task { await state.open(row.id) }
                            } label: {
                                RowView(row: row)
                            }
                            .buttonStyle(.plain)
                            .id(row.id)
                            Divider()
                        }
                    }
                }
                .onChange(of: state.selectedKey) { _, key in
                    if let key { scroller.scrollTo(key) }
                }
                }
            }

            Divider()

            HStack {
                Toggle("Launch at login", isOn: $login.isOn)
                    .toggleStyle(.checkbox)
                Spacer()
                Button("Quit") { NSApplication.shared.terminate(nil) }
                    .keyboardShortcut("q")
            }
            .padding(10)
        }
    }
}

struct RowView: View {
    let row: Row

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            // Colonne fixe : flèche de sélection et pictogrammes. Tout le
            // texte s'aligne ainsi sur un même bord, lignes des
            // vérifications comprises.
            HStack(spacing: 6) {
                Text(row.marker)
                    .foregroundStyle(.cyan)
                    .frame(width: 14)
                GlyphView(glyph: row.checks)
                GlyphView(glyph: row.review)
            }
            .font(.system(.body, design: .monospaced))

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(row.repo)
                        .foregroundStyle(.cyan)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Group {
                        Text(row.number).foregroundStyle(.secondary)
                        Text(row.age).foregroundStyle(.secondary)
                        Text(row.target).foregroundStyle(.purple)
                    }
                    .lineLimit(1)
                    .fixedSize()
                    ForEach(row.flags, id: \.self) { flag in
                        Text(flag).font(.caption).foregroundStyle(.orange).fixedSize()
                    }
                    Spacer(minLength: 0)
                }
                .font(.system(.body, design: .monospaced))
                Text(row.title)
                    .lineLimit(1)
                    .truncationMode(.tail)
                ForEach(Array(row.checkLines.enumerated()), id: \.offset) { _, line in
                    HStack(spacing: 6) {
                        Text(line.connector).foregroundStyle(.secondary)
                        GlyphView(glyph: line.glyph)
                        Text(line.name)
                            .lineLimit(1)
                            .truncationMode(.tail)
                    }
                    .font(.system(.caption, design: .monospaced))
                }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .contentShape(Rectangle())
    }
}

/// Raccourcis du clavier de la liste, comme dans la TUI : flèches pour la
/// sélection, Entrée ou → pour ouvrir, `m` pour fusionner. Des boutons
/// invisibles portent les raccourcis.
struct KeyboardShortcuts: View {
    @ObservedObject var state: BarState

    var body: some View {
        ZStack {
            Button("") { state.selectPrevious() }
                .keyboardShortcut(.upArrow, modifiers: [])
            Button("") { state.selectNext() }
                .keyboardShortcut(.downArrow, modifiers: [])
            Button("") { Task { await state.openSelection() } }
                .keyboardShortcut(.return, modifiers: [])
            Button("") { Task { await state.openSelection() } }
                .keyboardShortcut(.rightArrow, modifiers: [])
            Button("") { Task { await state.startMerge() } }
                .keyboardShortcut("m", modifiers: [])
        }
        .opacity(0)
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }
}
