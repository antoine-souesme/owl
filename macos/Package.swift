// swift-tools-version:5.10
import PackageDescription

let package = Package(
    name: "OwlBar",
    platforms: [.macOS(.v14)],
    targets: [
        .target(name: "OwlBarCore"),
        .executableTarget(name: "OwlBar", dependencies: ["OwlBarCore"]),
        .testTarget(
            name: "OwlBarCoreTests",
            dependencies: ["OwlBarCore"],
            resources: [.copy("Fixtures")]
        ),
    ]
)
