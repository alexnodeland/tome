// swift-tools-version: 5.9
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
    name: "TomeShell",
    platforms: [
        .macOS(.v12)
    ],
    products: [
        // Products define the executables and libraries a package produces.
        .library(
            name: "TomeShell",
            targets: ["TomeShell"]),
    ],
    dependencies: [
        // Dependencies declare other packages that this package depends on.
    ],
    targets: [
        // Targets are the basic building blocks of a package.
        .target(
            name: "TomeShell",
            dependencies: [],
            swiftSettings: [
                // Strict concurrency checking
                .enableExperimentalFeature("StrictConcurrency"),
                // Treat warnings as errors
                .unsafeFlags(["-warnings-as-errors"], .when(configuration: .release)),
            ]
        ),
        .testTarget(
            name: "TomeShellTests",
            dependencies: ["TomeShell"]),
    ]
)
