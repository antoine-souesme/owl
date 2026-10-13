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

            Divider()

            if let message = state.errorMessage {
                Text(message)
                    .textSelection(.enabled)
                    .multilineTextAlignment(.center)
                    .padding()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(state.rows) { row in
                            Button {
                                Task { await state.open(row.id) }
                            } label: {
                                RowView(row: row)
                            }
                            .buttonStyle(.plain)
                            Divider()
                        }
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
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 6) {
                GlyphView(glyph: row.checks)
                GlyphView(glyph: row.review)
                Text(row.repo).foregroundStyle(.cyan)
                Text(row.number).foregroundStyle(.secondary)
                Text(row.age).foregroundStyle(.secondary)
                Text(row.target).foregroundStyle(.purple)
                ForEach(row.flags, id: \.self) { flag in
                    Text(flag).font(.caption).foregroundStyle(.orange)
                }
                Spacer()
            }
            .font(.system(.body, design: .monospaced))
            Text(row.title)
                .lineLimit(1)
                .truncationMode(.tail)
                .padding(.leading, 40)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .contentShape(Rectangle())
    }
}
