# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Plasma effect: demoscene-style flowing color field (`plasma`, `plasma_with`, `plasma_effect`, `plasma_gradient_effect`)
- `storm` gradient preset (indigo → purple → orange → gold → amber → mahogany)
- `Color::luma()` for perceptual brightness using BT.709 coefficients
- `is_light_theme()` / `is_dark_theme()` convenience functions
- Terminal color detection: 5-tier fallback chain (OSC 10/11, COLORFGBG, TERM_PROGRAM, macOS system theme, defaults)
- `theme_adaptive` and `plasma` examples

### Changed

- Terminal color probing now uses a single `/dev/tty` session for both fg and bg queries
- RAII guard ensures terminal state is restored even on panic during color probing
- `ansi_index_to_color` now supports the full 0-255 range (color cube + grayscale ramp)
- Standalone animations now call `probe_colors()` before hiding cursor

### Fixed

- Empty palette no longer panics in `plasma()` (guards for 0 and 1 element palettes)
- Clippy warnings across codebase

## [0.1.0](https://github.com/jpwesselink/chromakopia/compare/v0.1.0-pr-4.82371ad...v0.1.0) - 2026-03-16

### Other

- Add fade_to_foreground/gradient/background on Animation
- Layered animation system with easing, rename to chromakopia
- Initial commit
