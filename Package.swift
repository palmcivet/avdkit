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
            url: "https://github.com/palmcivet/avdkit/releases/download/0.0.1/AvdkitFFI.xcframework.zip",
            checksum: "fcf39a989bcfc10eccc307bd3472c5e6252910cfc00bf524d913410156136862"
        ),
        .target(
            name: "Avdkit",
            dependencies: ["AvdkitFFI"],
            path: "Sources/Avdkit"
        ),
    ]
)
