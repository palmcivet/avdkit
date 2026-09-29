// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "Avdkit",
    platforms: [.macOS(.v13)],
    products: [
        .library(name: "Avdkit", targets: ["Avdkit"]),
    ],
    targets: [
        .binaryTarget(
            name: "AvdkitFFI",
            path: "Artifacts/AvdkitFFI.xcframework"
        ),
        .target(
            name: "Avdkit",
            dependencies: ["AvdkitFFI"],
            path: "Sources/Avdkit"
        ),
        .testTarget(
            name: "AvdkitTests",
            dependencies: ["Avdkit"],
            path: "Tests/AvdkitTests"
        ),
    ]
)
