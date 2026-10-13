// Annonce des pull requests devenues prêtes, avec la règle de la TUI
// (`App::announce_ready`) : rien à la première liste, une annonce par
// passage, un verdict inconnu ne change rien, une PR sortie de la liste est
// oubliée.
public struct ReadyTracker: Sendable {
    private var announced: Set<PrKey>?

    public init() {}

    /// Rend les pull requests à annoncer, dans l'ordre de la liste.
    public mutating func update(_ prs: [PullRequest]) -> [PullRequest] {
        guard var announced else {
            announced = Set(prs.filter { $0.mergeState == .clean }.map(\.key))
            return []
        }
        let present = Set(prs.map(\.key))
        announced.formIntersection(present)
        var newlyReady: [PullRequest] = []
        for pr in prs {
            switch pr.mergeState {
            case .clean:
                if announced.insert(pr.key).inserted { newlyReady.append(pr) }
            case .blocked:
                announced.remove(pr.key)
            case .unknown:
                break
            }
        }
        self.announced = announced
        return newlyReady
    }
}
