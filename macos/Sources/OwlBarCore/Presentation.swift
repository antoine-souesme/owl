// Tout ce que le panneau affiche, déjà décidé : pictogrammes, libellés,
// âges, colonnes. Les vues ne choisissent rien. Les choix suivent ceux de la
// TUI (`src/app/render.rs`) pour qu'on retrouve les mêmes repères.
import Foundation

public enum Tone: Sendable { case green, red, yellow, gray, cyan, blue }

/// Couleurs des colonnes, celles de la TUI (`src/app/render.rs`).
public enum Palette {
    public static let repo: Tone = .cyan
    public static let number: Tone = .gray
    public static let age: Tone = .gray
    public static let target: Tone = .blue
    public static let arrow: Tone = .gray
    public static let source: Tone = .blue
    public static let connector: Tone = .gray
    public static let sectionTitle: Tone = .cyan
}

public struct Glyph: Equatable, Sendable {
    public let symbol: String
    public let tone: Tone
}

/// Images de l'animation des vérifications en cours, celles de la TUI.
public let pendingFrames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]

public func checksGlyph(_ state: ChecksState, frame: Int = 0) -> Glyph {
    switch state {
    case .success: Glyph(symbol: "✓", tone: .green)
    case .failure: Glyph(symbol: "✗", tone: .red)
    case .pending: Glyph(symbol: pendingFrames[frame % pendingFrames.count], tone: .yellow)
    case .none: Glyph(symbol: "·", tone: .gray)
    }
}

public func reviewGlyph(_ state: ReviewState) -> Glyph {
    switch state {
    case .approved: Glyph(symbol: "✓", tone: .green)
    case .changesRequested: Glyph(symbol: "✗", tone: .red)
    case .reviewRequired: Glyph(symbol: "●", tone: .yellow)
    case .none: Glyph(symbol: "·", tone: .gray)
    }
}

public func methodLabel(_ method: MergeMethod) -> String {
    switch method {
    case .squash: "Squash and merge"
    case .rebase: "Rebase and merge"
    case .merge: "Create a merge commit"
    }
}

/// Minutes, puis heures, puis jours. Une date future donne zéro.
public func shortAge(from date: Date, now: Date) -> String {
    let minutes = max(0, Int(now.timeIntervalSince(date) / 60))
    if minutes < 60 { return "\(minutes)m" }
    let hours = minutes / 60
    if hours < 24 { return "\(hours)h" }
    return "\(hours / 24)d"
}

public struct Counter: Equatable, Sendable {
    public let ready: Int
    public let others: Int
}

/// « Prête » : GitHub dit que plus rien ne bloque. Tout le reste, brouillons
/// compris, compte dans le second nombre.
public func counter(_ prs: [PullRequest]) -> Counter {
    let ready = prs.filter { $0.mergeState == .clean }.count
    return Counter(ready: ready, others: prs.count - ready)
}

public func menuBarTitle(counter: Counter?, failed: Bool) -> String {
    if failed { return "🦉 ⚠" }
    guard let counter else { return "🦉 …" }
    return "🦉 \(counter.ready) · \(counter.others)"
}

/// Une vérification du dernier commit, sous la ligne de sa PR, raccordée
/// comme dans la TUI.
public struct CheckLine: Equatable, Sendable {
    public let connector: String
    public let glyph: Glyph
    public let name: String
}

/// Marqueur de la ligne sélectionnée, celui de la TUI.
public let selectionMarker = "→"

public struct Row: Equatable, Identifiable, Sendable {
    public let id: PrKey
    /// La ligne porte `selectionMarker`. Les autres gardent sa place, vide,
    /// pour que rien ne bouge quand la sélection change.
    public let isSelected: Bool
    public let repo: String
    public let number: String
    public let age: String
    public let target: String
    /// Flèche de fusion de la TUI, de la branche source vers la cible.
    public let arrow: String
    public let source: String
    public let title: String
    public let checks: Glyph
    public let review: Glyph
    public let flags: [String]
    public let checkLines: [CheckLine]
    /// Brouillon : toute la ligne est grisée, comme dans la TUI.
    public let dim: Bool
}

public func rows(_ prs: [PullRequest], now: Date, selected: PrKey? = nil, frame: Int = 0) -> [Row] {
    prs.map { pr in
        var flags: [String] = []
        if pr.isDraft { flags.append("draft") }
        if pr.mergeable == .conflicting { flags.append("⚠ conflicts") }
        return Row(
            id: pr.key,
            isSelected: pr.key == selected,
            repo: pr.repo,
            number: "#\(pr.number)",
            age: shortAge(from: pr.updatedAt, now: now),
            target: pr.baseRef,
            arrow: "←",
            source: pr.headRef,
            title: pr.title,
            checks: checksGlyph(pr.checks, frame: frame),
            review: reviewGlyph(pr.review),
            flags: flags,
            // Toutes les vérifications, dans l'ordre de GitHub.
            checkLines: pr.checkRuns.map {
                CheckLine(connector: "⎿", glyph: checksGlyph($0.state, frame: frame), name: $0.name)
            },
            dim: pr.isDraft
        )
    }
}

public let noMergeMethodMessage = "No merge method allowed on this repository."

public let readyTitle = "Ready to merge"

public func readyBody(_ pr: PullRequest) -> String {
    "\(pr.repo) #\(pr.number) · \(pr.title)"
}
