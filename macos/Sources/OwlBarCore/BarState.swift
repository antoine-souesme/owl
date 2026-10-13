// État du panneau. Il demande à `owl` et décide de ce qui s'affiche ; il ne
// lance aucun processus lui-même (c'est le travail de `OwlRunning`) et ne
// dessine rien.
import Foundation

public protocol Notifying: Sendable {
    func notify(title: String, body: String)
}

public enum DetailStatus: Equatable, Sendable {
    case loading
    case loaded(PrDetail)
    case failed(String)
}

/// Touches du panneau, traitées ici comme `App::handle` les traite dans la
/// TUI : les vues ne font que les transmettre.
public enum PanelKey: Sendable {
    case up, down, left, right, enter, escape, m, r
}

@MainActor
public final class BarState: ObservableObject {
    @Published public private(set) var pullRequests: [PullRequest] = []
    @Published public private(set) var loaded = false
    @Published public private(set) var errorMessage: String?
    @Published public private(set) var isRefreshing = false
    @Published public private(set) var lastRefresh: Date?
    @Published public private(set) var openKey: PrKey?
    @Published public private(set) var detail: DetailStatus = .loading
    @Published public private(set) var mergeDialog: MergeDialog?
    /// Message bref posé par une action, effacé à la touche suivante.
    @Published public private(set) var notice: String?
    @Published public private(set) var selectedKey: PrKey?
    /// Image courante de l'animation des vérifications en cours.
    @Published public private(set) var frame = 0
    public private(set) var preferredMethod: MergeMethod = .squash
    private var refreshInterval = 60
    private var pausedUntil: Date?
    private var tracker = ReadyTracker()
    /// Méthode de la dernière fusion réussie, présélectionnée ensuite.
    private var lastUsedMethod: MergeMethod?

    private let runner: OwlRunning
    private let notifier: Notifying
    private let now: () -> Date

    public init(runner: OwlRunning, notifier: Notifying, now: @escaping () -> Date = Date.init) {
        self.runner = runner
        self.notifier = notifier
        self.now = now
    }

    public var title: String {
        menuBarTitle(counter: loaded ? counter(pullRequests) : nil, failed: errorMessage != nil)
    }

    public var rows: [Row] { OwlBarCore.rows(pullRequests, now: now(), selected: selectedKey, frame: frame) }

    /// Une vérification tourne quelque part à l'écran : liste ou détail.
    public var isAnimating: Bool {
        let inList = pullRequests.contains {
            $0.checks == .pending || $0.checkRuns.contains { $0.state == .pending }
        }
        if inList { return true }
        if case .loaded(let detail) = detail {
            return detail.checks.contains { $0.state == .pending }
        }
        return false
    }

    /// Tour d'horloge de l'animation. Rien ne change, donc rien ne se
    /// redessine, quand aucune vérification ne tourne.
    public func tick() {
        if isAnimating { frame += 1 }
    }

    public func selectNext() {
        notice = nil
        moveSelection(by: 1)
    }

    public func selectPrevious() {
        notice = nil
        moveSelection(by: -1)
    }

    /// Déplace la sélection sans sortir de la liste.
    private func moveSelection(by step: Int) {
        guard !pullRequests.isEmpty else { return }
        let current = pullRequests.firstIndex { $0.key == selectedKey } ?? 0
        let next = min(max(current + step, 0), pullRequests.count - 1)
        selectedKey = pullRequests[next].key
    }

    /// Ouvre la PR sélectionnée.
    public func openSelection() async {
        guard let selectedKey else { return }
        await open(selectedKey)
    }

    public var mergeRendering: MergeRender? { mergeDialog.map(mergeRender) }

    /// Touche `m` : ouvre la fenêtre de fusion pour la PR affichée, celle du
    /// détail ou la sélection de la liste.
    public func startMerge() {
        notice = nil
        guard let key = openKey ?? selectedKey,
              let pr = (openKey == key ? openPullRequest : nil) ?? pullRequests.first(where: { $0.key == key })
        else { return }
        switch openMerge(pr, lastUsed: lastUsedMethod, preferred: preferredMethod) {
        case .dialog(let dialog): mergeDialog = dialog
        case .denied(let message): notice = message
        }
    }

    /// Échap : ferme la fenêtre, sauf pendant la fusion, qui est partie.
    public func cancelMerge() {
        guard let dialog = mergeDialog, dialog.stage != .submitting else { return }
        mergeDialog = nil
    }

    /// Entrée dans la fenêtre : passe l'avertissement, ou lance la fusion.
    public func confirmMerge() async {
        guard var dialog = mergeDialog else { return }
        switch dialog.stage {
        case .warning:
            dialog.stage = .choosing
            mergeDialog = dialog
        case .choosing, .failed:
            await submit(dialog)
        case .submitting:
            break
        }
    }

    public func selectMethod(_ index: Int) {
        guard var dialog = mergeDialog, dialog.stage == .choosing,
              dialog.choices.indices.contains(index), dialog.choices[index].allowed
        else { return }
        dialog.selected = index
        mergeDialog = dialog
    }

    private func submit(_ dialog: MergeDialog) async {
        guard let method = dialog.method else { return }
        var running = dialog
        running.stage = .submitting
        mergeDialog = running
        let key = dialog.key
        let result = await runner.run(["merge", key.repo, String(key.number), method.rawValue])
            .flatMap { decodeOwl(MergeData.self, from: $0) }
        guard mergeDialog?.key == key else { return }
        switch result {
        case .success:
            lastUsedMethod = method
            mergeDialog = nil
            notice = "\(key.repo) #\(key.number) merged"
            // La PR fusionnée quitte la liste sans attendre la relecture,
            // comme dans la TUI.
            pullRequests.removeAll { $0.key == key }
            if !pullRequests.contains(where: { $0.key == selectedKey }) {
                selectedKey = pullRequests.first?.key
            }
            if openKey == key { openKey = nil }
            await refresh()
        case .failure(let failure):
            running.stage = .failed(failure.message)
            mergeDialog = running
        }
    }

    /// Aiguille une touche, comme la TUI : la fenêtre de fusion d'abord,
    /// puis les touches communes, puis celles de la vue affichée.
    public func press(_ key: PanelKey) async {
        if var dialog = mergeDialog {
            switch key {
            case .up, .down:
                if dialog.stage == .choosing {
                    dialog.move(down: key == .down)
                    mergeDialog = dialog
                }
            case .enter: await confirmMerge()
            case .escape: cancelMerge()
            default: break
            }
            return
        }
        switch key {
        case .r:
            notice = nil
            await refresh()
            return
        case .m:
            startMerge()
            return
        default: break
        }
        if openKey != nil {
            switch key {
            case .left, .escape: back()
            default: break
            }
        } else {
            switch key {
            case .up: selectPrevious()
            case .down: selectNext()
            case .enter, .right: await openSelection()
            default: break
            }
        }
    }

    /// « Updated 3m ago », ou « Updating… » pendant une lecture.
    public var refreshLabel: String {
        if isRefreshing { return "Updating…" }
        guard let lastRefresh else { return "" }
        return "Updated \(shortAge(from: lastRefresh, now: now())) ago"
    }

    /// Délai avant la prochaine lecture automatique ; `nil` quand la
    /// configuration la désactive.
    public var nextRefreshDelay: TimeInterval? {
        refreshInterval > 0 ? TimeInterval(refreshInterval) : nil
    }

    public var openPullRequest: PullRequest? {
        guard let openKey else { return nil }
        if case .loaded(let detail) = detail, detail.summary.key == openKey { return detail.summary }
        return pullRequests.first { $0.key == openKey }
    }

    public var detailHeaderLines: [String] {
        if case .loaded(let detail) = detail { return detailHeader(detail) }
        return []
    }

    public var detailSectionList: [DetailSection] {
        if case .loaded(let detail) = detail { return detailSections(detail, now: now(), frame: frame) }
        return []
    }

    /// Méthodes à proposer pour la PR ouverte, et celle présélectionnée.
    public var mergeOptions: (choices: [MergeMethod], selected: MergeMethod?) {
        guard let pr = openPullRequest else { return ([], nil) }
        let choices = mergeChoices(pr)
        return (choices, preselected(choices, preferred: preferredMethod))
    }

    public func refresh() async {
        guard !isRefreshing else { return }
        if let pausedUntil, now() < pausedUntil { return }
        isRefreshing = true
        defer { isRefreshing = false }

        let result = await runner.run(["list"]).flatMap { decodeOwl(ListData.self, from: $0) }
        switch result {
        case .success(let list):
            pullRequests = list.pullRequests
            // La sélection suit sa PR ; si elle est sortie de la liste, elle
            // revient en tête.
            if !list.pullRequests.contains(where: { $0.key == selectedKey }) {
                selectedKey = list.pullRequests.first?.key
            }
            preferredMethod = list.preferredMergeMethod
            refreshInterval = list.refreshInterval
            pausedUntil = nil
            errorMessage = nil
            loaded = true
            lastRefresh = now()
            for pr in tracker.update(list.pullRequests) {
                notifier.notify(title: readyTitle, body: readyBody(pr))
            }
        case .failure(let failure):
            errorMessage = failure.message
            if failure.kind == "rate_limited" { pausedUntil = failure.resetAt }
        }
    }

    public func open(_ key: PrKey) async {
        notice = nil
        openKey = key
        selectedKey = key
        detail = .loading
        let result = await runner.run(["detail", key.repo, String(key.number)])
            .flatMap { decodeOwl(PrDetail.self, from: $0) }
        // Retour à la liste ou autre PR ouverte entre-temps : cette réponse
        // ne concerne plus l'écran.
        guard openKey == key else { return }
        switch result {
        case .success(let value): detail = .loaded(value)
        case .failure(let failure): detail = .failed(failure.message)
        }
    }

    public func back() {
        notice = nil
        openKey = nil
    }
}
