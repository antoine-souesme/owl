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
            }
            .padding(10)

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
                Text(selectionMarker)
                    .opacity(row.isSelected ? 1 : 0)
                    .frame(width: 14)
                GlyphView(glyph: row.checks)
                GlyphView(glyph: row.review)
            }
            .font(.system(.body, design: .monospaced))

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(row.title)
                        .lineLimit(1)
                        .truncationMode(.tail)
                    Group {
                        Text(row.number).foregroundStyle(Palette.number.color)
                        Text(row.age).foregroundStyle(Palette.age.color)
                    }
                    .font(.system(.body, design: .monospaced))
                    .lineLimit(1)
                    .fixedSize()
                    ForEach(row.flags, id: \.self) { flag in
                        Text(flag).font(.caption).fixedSize()
                    }
                    Spacer(minLength: 0)
                }
                HStack(spacing: 6) {
                    Text(row.target).foregroundStyle(Palette.target.color).fixedSize()
                    Text(row.arrow).foregroundStyle(Palette.arrow.color).fixedSize()
                    Text(row.source)
                        .foregroundStyle(Palette.source.color)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                .font(.system(.callout, design: .monospaced))
                Text(row.repo)
                    .foregroundStyle(Palette.repo.color)
                    .font(.system(.callout, design: .monospaced))
                    .lineLimit(1)
                    .truncationMode(.middle)
                ForEach(Array(row.checkLines.enumerated()), id: \.offset) { _, line in
                    HStack(spacing: 6) {
                        Text(line.connector).foregroundStyle(Palette.connector.color)
                        GlyphView(glyph: line.glyph)
                        Text(line.name)
                            .lineLimit(1)
                            .truncationMode(.tail)
                    }
                    .font(.system(.caption, design: .monospaced))
                }
            }
        }
        .opacity(row.dim ? 0.5 : 1)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .contentShape(Rectangle())
    }
}
