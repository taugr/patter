// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "PatterParakeet",
    platforms: [.macOS(.v15)],
    dependencies: [
        .package(url: "https://github.com/FluidInference/FluidAudio.git", exact: "0.17.1", traits: [])
    ],
    targets: [
        .executableTarget(name: "PatterParakeet", dependencies: [.product(name: "FluidAudio", package: "FluidAudio")])
    ]
)
