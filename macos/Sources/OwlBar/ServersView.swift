import OwlBarCore
import SwiftUI

/// Onglet Servers : champ de recherche, lignes, message bref et fenêtre de
/// confirmation. Tout ce qui s'affiche vient de `ServersState`.
struct ServersView: View {
    @ObservedObject var servers: ServersState

    var body: some View {
        VStack(spacing: 0) {
            if let message = servers.placeholder {
                Text(message)
                    .textSelection(.enabled)
                    .multilineTextAlignment(.center)
                    .padding()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(servers.rows) { row in
                            ServerRowView(row: row) {
                                Task { await servers.stop(row.id) }
                            }
                            Divider()
                        }
                    }
                }
                .frame(maxHeight: .infinity)
            }

            if let notice = servers.notice {
                Divider()
                Text(notice)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(10)
            }
        }
        .overlay {
            if let render = servers.dialogRendering {
                DialogOverlay(
                    render: render,
                    onConfirm: { Task { await servers.confirmStop() } },
                    onCancel: { servers.cancelStop() }
                )
            }
        }
    }
}

/// Bout droit de la ligne des onglets, côté serveurs : recherche et
/// rechargement.
struct ServersHeader: View {
    @ObservedObject var servers: ServersState

    var body: some View {
        TextField("Port", text: $servers.query)
            .textFieldStyle(.roundedBorder)
            .frame(width: 90)
            .onSubmit { Task { await servers.submitSearch() } }
            .onChange(of: servers.query) { Task { await servers.queryChanged() } }
        Button {
            Task { await servers.reload() }
        } label: {
            Image(systemName: "arrow.clockwise")
        }
        .buttonStyle(.borderless)
        .disabled(servers.isLoading)
    }
}

struct ServerRowView: View {
    let row: ServerRow
    let stop: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 10) {
            Text(row.port)
                .font(.system(.title3, design: .monospaced).bold())
                .frame(minWidth: 60, alignment: .leading)
            VStack(alignment: .leading, spacing: 2) {
                Text(row.project)
                    .foregroundStyle(Palette.repo.color)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .help(row.directory ?? "")
                HStack(spacing: 6) {
                    Text(row.command)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Text(row.age)
                        .foregroundStyle(Palette.age.color)
                        .fixedSize()
                }
                .font(.system(.callout, design: .monospaced))
            }
            Spacer(minLength: 0)
            Button(row.stopLabel, action: stop)
                .disabled(row.isStopping)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }
}
