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
            url: "https://github.com/palmcivet/avdkit/releases/download/0.0.2/AvdkitFFI.xcframework.zip",
            checksum: "31366989e1c404b798bbd657301d65c2e5050af2e8c903a1b734f0f2c5a0e5b5"
        ),
        .target(
            name: "Avdkit",
            dependencies: ["AvdkitFFI"],
            path: "Sources/Avdkit"
        ),
    ]
)
