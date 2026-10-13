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

public enum MergeStatus: Equatable, Sendable {
    case idle
    case running
    case failed(String)
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
    @Published public private(set) var mergeStatus: MergeStatus = .idle
    @Published public private(set) var selectedKey: PrKey?
    /// Image courante de l'animation des vérifications en cours.
    @Published public private(set) var frame = 0
    /// La fusion de la PR ouverte attend une confirmation.
    @Published public private(set) var confirmingMerge = false
    public private(set) var preferredMethod: MergeMethod = .squash
    private var refreshInterval = 60
    private var pausedUntil: Date?
    private var tracker = ReadyTracker()
    /// Fusions en cours, par PR : l'état affiché ne vaut que pour la PR
    /// ouverte, même si l'on navigue pendant qu'une fusion tourne.
    private var merging: Set<PrKey> = []

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

    public func selectNext() { moveSelection(by: 1) }

    public func selectPrevious() { moveSelection(by: -1) }

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

    /// Touche `m` de la liste : ouvre la PR sélectionnée directement sur la
    /// confirmation de fusion.
    public func startMerge() async {
        guard let selectedKey else { return }
        await open(selectedKey, confirmingMerge: true)
    }

    /// Demande la confirmation de fusion de la PR ouverte, si le dépôt
    /// autorise au moins une méthode.
    public func requestMerge() {
        guard openKey != nil, mergeStatus != .running, !mergeOptions.choices.isEmpty else { return }
        confirmingMerge = true
    }

    public func cancelMerge() {
        confirmingMerge = false
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

    public func open(_ key: PrKey, confirmingMerge confirm: Bool = false) async {
        openKey = key
        selectedKey = key
        detail = .loading
        mergeStatus = merging.contains(key) ? .running : .idle
        confirmingMerge = false
        if confirm { requestMerge() }
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
        openKey = nil
        mergeStatus = .idle
        confirmingMerge = false
    }

    public func merge(_ method: MergeMethod) async {
        guard let key = openKey, !merging.contains(key) else { return }
        merging.insert(key)
        mergeStatus = .running
        confirmingMerge = false
        let result = await runner.run(["merge", key.repo, String(key.number), method.rawValue])
            .flatMap { decodeOwl(MergeData.self, from: $0) }
        merging.remove(key)
        // Autre écran entre-temps : le résultat ne concerne plus ce qui est
        // affiché. La liste est rafraîchie quand même après un succès.
        let shown = openKey == key
        switch result {
        case .success:
            if shown {
                mergeStatus = .idle
                openKey = nil
            }
            await refresh()
        case .failure(let failure):
            if shown { mergeStatus = .failed(failure.message) }
        }
    }
}
