# Tagliacarte Flutter UI

This app is the replacement frontend for Tagliacarte.

## Platforms

- Android: supported, build with `flutter build apk`
- iOS: supported (requires full Xcode installation)
- macOS: supported (requires full Xcode installation)
- Linux: supported

## Rust integration

`flutter_rust_bridge.yaml` in the repository root configures binding generation
for `tagliacarte_app` (`app/src/lib.rs`).

## Run

```bash
flutter pub get
flutter run
```

## Tests

```bash
flutter test
flutter test integration_test
```
# flutter_ui

A new Flutter project.

## Getting Started

This project is a starting point for a Flutter application.

A few resources to get you started if this is your first Flutter project:

- [Learn Flutter](https://docs.flutter.dev/get-started/learn-flutter)
- [Write your first Flutter app](https://docs.flutter.dev/get-started/codelab)
- [Flutter learning resources](https://docs.flutter.dev/reference/learning-resources)

For help getting started with Flutter development, view the
[online documentation](https://docs.flutter.dev/), which offers tutorials,
samples, guidance on mobile development, and a full API reference.
