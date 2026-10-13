// Fenêtre de fusion, avec les règles et les textes de la TUI
// (`App::open_merge`, `App::merge_render`) : refus expliqués, avertissement
// avant une fusion directe sur une branche protégée, choix de la méthode,
// fusion en cours, échec avec le message de GitHub tel quel.

/// Branche visée pour laquelle la fusion commence par un avertissement,
/// comme dans la TUI.
public let warnedBase = "main"

public let deniedDraft = "This pull request is a draft, it must be published first."
public let deniedConflicts = "Conflicts to resolve."
public let deniedUnknownState = "Merge state being computed, try again in a moment."

public struct MergeChoice: Equatable, Sendable {
    public let method: MergeMethod
    public let allowed: Bool
}

public enum MergeStage: Equatable, Sendable {
    case warning
    case choosing
    case submitting
    case failed(String)
}

public struct MergeDialog: Equatable, Sendable {
    public let key: PrKey
    public let title: String
    public let baseRef: String
    public let headRef: String
    public let choices: [MergeChoice]
    public internal(set) var selected: Int
    public internal(set) var stage: MergeStage

    /// Méthode sélectionnée, si le dépôt l'autorise.
    public var method: MergeMethod? {
        choices.indices.contains(selected) && choices[selected].allowed ? choices[selected].method : nil
    }

    /// Monte ou descend jusqu'à la méthode autorisée suivante ; reste sur
    /// place au bord de la liste.
    mutating func move(down: Bool) {
        var index = selected
        while true {
            index += down ? 1 : -1
            guard choices.indices.contains(index) else { return }
            if choices[index].allowed {
                selected = index
                return
            }
        }
    }
}

public enum MergeOpening: Equatable, Sendable {
    case dialog(MergeDialog)
    case denied(String)
}

/// Ouvre la fenêtre pour cette PR, ou dit pourquoi elle ne s'ouvre pas.
public func openMerge(_ pr: PullRequest, lastUsed: MergeMethod?, preferred: MergeMethod) -> MergeOpening {
    if pr.isDraft { return .denied(deniedDraft) }
    switch pr.mergeable {
    case .conflicting: return .denied(deniedConflicts)
    case .unknown: return .denied(deniedUnknownState)
    case .mergeable: break
    }
    let choices = MergeMethod.allCases.map { MergeChoice(method: $0, allowed: pr.mergeMethods.contains($0)) }
    guard choices.contains(where: \.allowed) else { return .denied(noMergeMethodMessage) }
    // La dernière méthode utilisée, puis celle des réglages, puis la première
    // autorisée.
    let selected = [lastUsed, preferred].compactMap { $0 }
        .compactMap { wanted in choices.firstIndex { $0.allowed && $0.method == wanted } }
        .first ?? choices.firstIndex(where: \.allowed)!
    return .dialog(MergeDialog(
        key: pr.key,
        title: pr.title,
        baseRef: pr.baseRef,
        headRef: pr.headRef,
        choices: choices,
        selected: selected,
        stage: pr.baseRef == warnedBase ? .warning : .choosing
    ))
}

/// Morceau de texte avec son ton ; `nil` : couleur par défaut.
public struct Segment: Equatable, Sendable {
    public let text: String
    public let tone: Tone?

    public init(text: String, tone: Tone?) {
        self.text = text
        self.tone = tone
    }
}

public struct MergeRender: Equatable, Sendable {
    public let title: String
    /// Couleur du cadre ; `nil` : cadre ordinaire.
    public let frameTone: Tone?
    public let lines: [[Segment]]
    /// Boutons pour la souris, mêmes effets qu'Entrée et Échap ; `nil` :
    /// pas de bouton, la fusion est partie.
    public let confirmLabel: String?
    public let cancelLabel: String?
}

private func shortMethodLabel(_ method: MergeMethod) -> String {
    methodLabel(method).prefix(1).lowercased() + methodLabel(method).dropFirst()
}

public func mergeRender(_ dialog: MergeDialog) -> MergeRender {
    let warning = dialog.stage == .warning
    var lines: [[Segment]] = [
        [
            Segment(text: "#\(dialog.key.number)", tone: Palette.number),
            Segment(text: " ", tone: nil),
            Segment(text: dialog.title, tone: nil),
        ],
        [
            // Dans l'avertissement, la branche visée passe au rouge : c'est
            // elle qui justifie la fenêtre.
            Segment(text: dialog.baseRef, tone: warning ? .red : Palette.target),
            Segment(text: " ← ", tone: Palette.arrow),
            Segment(text: dialog.headRef, tone: Palette.source),
        ],
        [],
    ]
    func plain(_ text: String) -> [Segment] { [Segment(text: text, tone: nil)] }
    func red(_ text: String) -> [Segment] { [Segment(text: text, tone: .red)] }

    switch dialog.stage {
    case .warning:
        lines.append(red("⚠  DIRECT MERGE INTO \(dialog.baseRef.uppercased())  ⚠"))
        lines.append([])
        lines.append(red("This pull request targets \(dialog.baseRef). Merging it will land directly on \(dialog.baseRef)."))
        lines.append([])
        lines.append(plain("Enter to continue · Esc to cancel"))
    case .choosing:
        lines.append(plain("Method:"))
        for (index, choice) in dialog.choices.enumerated() {
            let marker = index == dialog.selected ? "\(selectionMarker) " : "  "
            let text = "  \(marker)\(methodLabel(choice.method))"
            lines.append([Segment(text: text, tone: choice.allowed ? nil : .gray)])
        }
        lines.append([])
        lines.append(plain("Enter to confirm · Esc to cancel"))
    case .submitting:
        if let method = dialog.method {
            lines.append(plain("Method: \(shortMethodLabel(method))"))
        }
        lines.append([])
        lines.append(plain("Merging…"))
    case .failed(let message):
        lines.append(red(message))
        lines.append([])
        lines.append(plain("Enter to retry · Esc to close"))
    }
    let buttons: (confirm: String?, cancel: String?) = switch dialog.stage {
    case .warning: ("Continue", "Cancel")
    case .choosing: (dialog.method.map(methodLabel), "Cancel")
    case .submitting: (nil, nil)
    case .failed: ("Retry", "Close")
    }
    return MergeRender(
        title: warning ? "WARNING" : "Merge",
        frameTone: warning ? .red : nil,
        lines: lines,
        confirmLabel: buttons.confirm,
        cancelLabel: buttons.cancel
    )
}
