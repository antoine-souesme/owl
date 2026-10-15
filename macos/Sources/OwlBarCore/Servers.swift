// Onglet Servers : ce qui s'affiche et ce que fait chaque geste, déjà
// décidés. L'état demande à `owl` (`ports`, `stop`) par `OwlRunning` et ne
// lance rien lui-même ; les vues ne font que dessiner.
import Foundation

public let noServerMessage = "No node server running"

public func nothingListensMessage(port: Int) -> String {
    "Nothing listens on port \(port)"
}

public func hiddenMessage(port: Int) -> String {
    "Port \(port) is used by a process owl cannot see (system or another user)."
}

public func notAPortMessage(_ text: String) -> String {
    "Not a port: \(text)"
}

/// Un port de 1 à 65535, espaces autour tolérés.
public func parsePort(_ text: String) -> Int? {
    let trimmed = text.trimmingCharacters(in: .whitespaces)
    guard !trimmed.isEmpty, trimmed.allSatisfy(\.isASCII), let port = Int(trimmed),
          (1...65535).contains(port)
    else { return nil }
    return port
}

public struct ServerRow: Equatable, Identifiable, Sendable {
    public let id: ServerId
    public let port: String
    public let project: String
    /// Chemin complet, montré au survol du projet.
    public let directory: String?
    public let command: String
    /// Durée depuis le démarrage, au format de l'âge des PR.
    public let age: String
    public let stopLabel: String
    public let isStopping: Bool
}

public func serverRows(_ servers: [Server], now: Date, stopping: Set<ServerId>) -> [ServerRow] {
    servers.map { server in
        let isStopping = stopping.contains(server.id)
        return ServerRow(
            id: server.id,
            port: String(server.port),
            project: server.project ?? "—",
            directory: server.directory,
            command: server.command,
            age: shortAge(from: server.startedAt, now: now),
            stopLabel: isStopping ? "Stopping…" : "Stop",
            isStopping: isStopping
        )
    }
}

/// Fenêtre de confirmation d'un arrêt, ouverte depuis une recherche.
public struct StopDialog: Equatable, Sendable {
    public enum Stage: Equatable, Sendable {
        case confirming
        case stopping
        case failed(String)
    }

    public let server: Server
    public var stage: Stage
}

/// Même forme que la fenêtre de fusion : la vue est commune.
public func stopRender(_ dialog: StopDialog) -> MergeRender {
    let server = dialog.server
    var lines: [[Segment]] = [
        [Segment(text: server.project ?? server.command, tone: Palette.repo)],
    ]
    if let directory = server.directory {
        lines.append([Segment(text: directory, tone: .gray)])
    }
    if let critical = server.critical {
        lines.append([Segment(text: "Critical: \(critical.reason)", tone: .red)])
    }
    if case .failed(let message) = dialog.stage {
        lines.append([Segment(text: message, tone: .red)])
    }
    let busy = dialog.stage == .stopping
    return MergeRender(
        title: "Stop \(server.command) on port \(server.port)?",
        frameTone: server.critical == nil ? nil : .red,
        lines: lines,
        confirmLabel: busy ? nil : "Stop",
        cancelLabel: busy ? nil : "Cancel"
    )
}

/// Recherche affichée à la place de la liste.
public enum SearchState: Equatable, Sendable {
    case invalid(String)
    case found(port: Int, servers: [Server], hidden: Bool)
}

@MainActor
public final class ServersState: ObservableObject {
    /// Texte du champ « Port », lié à la vue.
    @Published public var query = ""
    @Published public private(set) var servers: [Server] = []
    @Published public private(set) var loaded = false
    @Published public private(set) var errorMessage: String?
    @Published public private(set) var isLoading = false
    @Published public private(set) var search: SearchState?
    @Published public private(set) var stopping: Set<ServerId> = []
    @Published public private(set) var dialog: StopDialog?
    /// Message bref d'un arrêt raté depuis la liste.
    @Published public private(set) var notice: String?

    private let runner: OwlRunning
    private let now: () -> Date

    /// Ce qu'une lecture doit rapporter : la liste, un port, ou rien quand le
    /// champ contient autre chose qu'un port.
    private enum Target: Equatable {
        case list, port(Int), none
    }

    /// Ce que l'écran attend en ce moment.
    private var target: Target = .list
    /// Lecture en cours, s'il y en a une.
    private var reading: Target?
    /// Une demande est arrivée pendant la lecture : son résultat est périmé
    /// et il faut relire.
    private var readAgain = false

    public init(runner: OwlRunning, now: @escaping () -> Date = Date.init) {
        self.runner = runner
        self.now = now
    }

    public var rows: [ServerRow] { serverRows(displayed, now: now(), stopping: stopping) }

    private var displayed: [Server] {
        if case .found(_, let found, _) = search { return found }
        return servers
    }

    /// Message à la place des lignes, ou `nil` quand les lignes suffisent.
    public var placeholder: String? {
        if let errorMessage { return errorMessage }
        switch search {
        case .invalid(let text):
            return notAPortMessage(text)
        case .found(let port, let found, let hidden):
            guard found.isEmpty else { return nil }
            return hidden ? hiddenMessage(port: port) : nothingListensMessage(port: port)
        case nil:
            return loaded && servers.isEmpty ? noServerMessage : nil
        }
    }

    public var dialogRendering: MergeRender? { dialog.map(stopRender) }

    /// Relit ce qui est affiché : la recherche en cours, sinon la liste.
    public func reload() async {
        await read(target)
    }

    /// Entrée dans le champ : un port valide lance la recherche, un champ
    /// vide revient à la liste.
    public func submitSearch() async {
        notice = nil
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        if trimmed.isEmpty {
            await clearSearch()
            return
        }
        guard let port = parsePort(trimmed) else {
            target = .none
            search = .invalid(query)
            errorMessage = nil
            return
        }
        await read(.port(port))
    }

    /// Le champ a changé : vidé, il ramène la liste.
    public func queryChanged() async {
        if query.trimmingCharacters(in: .whitespaces).isEmpty, search != nil {
            await clearSearch()
        }
    }

    public func clearSearch() async {
        query = ""
        search = nil
        notice = nil
        errorMessage = nil
        await read(.list)
    }

    /// Bouton Stop d'une ligne. Depuis la liste, l'arrêt part aussitôt ;
    /// depuis une recherche, il passe toujours par la confirmation.
    public func stop(_ id: ServerId) async {
        notice = nil
        if case .found(_, let found, _) = search {
            guard let server = found.first(where: { $0.id == id }) else { return }
            dialog = StopDialog(server: server, stage: .confirming)
            return
        }
        guard servers.contains(where: { $0.id == id }), !stopping.contains(id) else { return }
        stopping.insert(id)
        let result = await send(id)
        stopping.remove(id)
        if case .failure(let failure) = result { notice = failure.message }
        await read(.list, fresh: true)
    }

    /// Entrée ou bouton Stop dans la fenêtre.
    public func confirmStop() async {
        guard var current = dialog, current.stage != .stopping else { return }
        current.stage = .stopping
        dialog = current
        let result = await send(current.server.id)
        guard dialog?.server.id == current.server.id else { return }
        switch result {
        case .success:
            dialog = nil
            await read(target, fresh: true)
        case .failure(let failure):
            current.stage = .failed(failure.message)
            dialog = current
        }
    }

    /// Échap, Cancel ou clic hors de la fenêtre ; sans effet pendant
    /// l'arrêt, qui est parti.
    public func cancelStop() {
        guard let current = dialog, current.stage != .stopping else { return }
        dialog = nil
    }

    /// Touches de l'onglet : la fenêtre d'abord, puis la recherche.
    public func press(_ key: PanelKey) async {
        if dialog != nil {
            switch key {
            case .enter: await confirmStop()
            case .escape: cancelStop()
            default: break
            }
            return
        }
        switch key {
        case .enter: await submitSearch()
        case .escape: if search != nil || !query.isEmpty { await clearSearch() }
        default: break
        }
    }

    private func send(_ id: ServerId) async -> Result<StopData, OwlFailure> {
        await runner.run(["stop", String(id.pid), String(id.port)])
            .flatMap { decodeOwl(StopData.self, from: $0) }
    }

    /// Une seule lecture à la fois. Une demande qui arrive pendant une
    /// lecture n'en lance pas une seconde : elle rend la lecture en cours
    /// périmée, et la lecture recommence avec ce que l'écran attend alors.
    /// `fresh` : l'état a changé (un serveur vient d'être coupé), même une
    /// lecture de la même chose est périmée.
    private func read(_ wanted: Target, fresh: Bool = false) async {
        target = wanted
        if let reading {
            if reading != wanted || fresh { readAgain = true }
            return
        }
        guard wanted != .none else { return }
        isLoading = true
        defer { isLoading = false }
        repeat {
            readAgain = false
            let current = target
            reading = current
            let arguments: [String]
            switch current {
            case .list: arguments = ["ports"]
            case .port(let port): arguments = ["ports", String(port)]
            case .none:
                reading = nil
                return
            }
            let result = await runner.run(arguments).flatMap { decodeOwl(PortsData.self, from: $0) }
            reading = nil
            guard !readAgain, current == target else { continue }
            switch (result, current) {
            case (.success(let data), .port(let port)):
                search = .found(port: port, servers: data.servers, hidden: data.hidden)
                errorMessage = nil
            case (.success(let data), _):
                servers = data.servers
                errorMessage = nil
                loaded = true
            case (.failure(let failure), _):
                errorMessage = failure.message
            }
        } while readAgain
    }
}
