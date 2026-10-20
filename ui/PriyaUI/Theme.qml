// Generated from ui/design/tokens.json by `cargo xtask tokens`; do not edit.
pragma Singleton
import QtQuick

QtObject {
    id: theme

    // Which palette is shown: "dark", "light", "oled".
    property string mode: "dark"
    // Movement off (the user's setting or the system's): changes are instant.
    property bool reducedMotion: false
    readonly property bool animationsEnabled: !reducedMotion
    // Interface scale; every size below is multiplied by it.
    property real scale: 1.0

    readonly property QtObject dark: QtObject {
        readonly property color accent: "#F2B544"
        readonly property color accentSubtle: "#3A2C10"
        readonly property color bgBase: "#121214"
        readonly property color bgSurface1: "#1A1A1F"
        readonly property color bgSurface2: "#222229"
        readonly property color bgSurface3: "#2B2B33"
        readonly property color borderStrong: "#75757F"
        readonly property color borderSubtle: "#34343D"
        readonly property color danger: "#F5675C"
        readonly property color focus: "#F2B544"
        readonly property color info: "#4AA3F0"
        readonly property color modeDirect: "#F08A24"
        readonly property color modeDirectRaw: "#F5675C"
        readonly property color modeLook: "#4AA3F0"
        readonly property color modeSandbox: "#3FB97A"
        readonly property color textOnAccent: "#1A1300"
        readonly property color success: "#3FB97A"
        readonly property color textPrimary: "#EDEDF0"
        readonly property color textSecondary: "#A9A9B4"
        readonly property color textTertiary: "#9A9AA5"
        readonly property color warning: "#F08A24"
        readonly property color elevationCardBorder: "#34343D"
        readonly property color elevationCardShadow1: "#00000000"
        readonly property real elevationCardShadow1Y: 0
        readonly property real elevationCardShadow1Blur: 0
        readonly property color elevationCardShadow2: "#00000000"
        readonly property real elevationCardShadow2Y: 0
        readonly property real elevationCardShadow2Blur: 0
    }
    readonly property QtObject light: QtObject {
        readonly property color accent: "#915500"
        readonly property color accentSubtle: "#FFF1D6"
        readonly property color bgBase: "#FAFAF7"
        readonly property color bgSurface1: "#FFFFFF"
        readonly property color bgSurface2: "#F3F3EF"
        readonly property color bgSurface3: "#EAEAE4"
        readonly property color borderStrong: "#83837B"
        readonly property color borderSubtle: "#DCDCD4"
        readonly property color danger: "#C42B1C"
        readonly property color focus: "#915500"
        readonly property color info: "#1C66AA"
        readonly property color modeDirect: "#A14B00"
        readonly property color modeDirectRaw: "#C42B1C"
        readonly property color modeLook: "#1C66AA"
        readonly property color modeSandbox: "#1A7446"
        readonly property color textOnAccent: "#FFFFFF"
        readonly property color success: "#1A7446"
        readonly property color textPrimary: "#18181B"
        readonly property color textSecondary: "#55555E"
        readonly property color textTertiary: "#64646D"
        readonly property color warning: "#A14B00"
        readonly property color elevationCardBorder: "#00000000"
        readonly property color elevationCardShadow1: "#18181B0F"
        readonly property real elevationCardShadow1Y: 1
        readonly property real elevationCardShadow1Blur: 2
        readonly property color elevationCardShadow2: "#18181B0F"
        readonly property real elevationCardShadow2Y: 4
        readonly property real elevationCardShadow2Blur: 12
    }
    readonly property QtObject oled: QtObject {
        readonly property color accent: "#F2B544"
        readonly property color accentSubtle: "#2A1F08"
        readonly property color bgBase: "#000000"
        readonly property color bgSurface1: "#0B0B0D"
        readonly property color bgSurface2: "#151518"
        readonly property color bgSurface3: "#1E1E23"
        readonly property color borderStrong: "#6C6C78"
        readonly property color borderSubtle: "#26262C"
        readonly property color danger: "#F5675C"
        readonly property color focus: "#F2B544"
        readonly property color info: "#4AA3F0"
        readonly property color modeDirect: "#F08A24"
        readonly property color modeDirectRaw: "#F5675C"
        readonly property color modeLook: "#4AA3F0"
        readonly property color modeSandbox: "#3FB97A"
        readonly property color textOnAccent: "#1A1300"
        readonly property color success: "#3FB97A"
        readonly property color textPrimary: "#EDEDF0"
        readonly property color textSecondary: "#A9A9B4"
        readonly property color textTertiary: "#9A9AA5"
        readonly property color warning: "#F08A24"
        readonly property color elevationCardBorder: "#26262C"
        readonly property color elevationCardShadow1: "#00000000"
        readonly property real elevationCardShadow1Y: 0
        readonly property real elevationCardShadow1Blur: 0
        readonly property color elevationCardShadow2: "#00000000"
        readonly property real elevationCardShadow2Y: 0
        readonly property real elevationCardShadow2Blur: 0
    }
    readonly property QtObject palette: ({ "dark": dark, "light": light, "oled": oled })[mode] ?? dark

    // The colours in use. They change smoothly when the mode does.
    property color accent: palette.accent
    Behavior on accent { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color accentSubtle: palette.accentSubtle
    Behavior on accentSubtle { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color bgBase: palette.bgBase
    Behavior on bgBase { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color bgSurface1: palette.bgSurface1
    Behavior on bgSurface1 { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color bgSurface2: palette.bgSurface2
    Behavior on bgSurface2 { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color bgSurface3: palette.bgSurface3
    Behavior on bgSurface3 { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color borderStrong: palette.borderStrong
    Behavior on borderStrong { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color borderSubtle: palette.borderSubtle
    Behavior on borderSubtle { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color danger: palette.danger
    Behavior on danger { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color focus: palette.focus
    Behavior on focus { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color info: palette.info
    Behavior on info { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color modeDirect: palette.modeDirect
    Behavior on modeDirect { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color modeDirectRaw: palette.modeDirectRaw
    Behavior on modeDirectRaw { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color modeLook: palette.modeLook
    Behavior on modeLook { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color modeSandbox: palette.modeSandbox
    Behavior on modeSandbox { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color textOnAccent: palette.textOnAccent
    Behavior on textOnAccent { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color success: palette.success
    Behavior on success { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color textPrimary: palette.textPrimary
    Behavior on textPrimary { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color textSecondary: palette.textSecondary
    Behavior on textSecondary { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color textTertiary: palette.textTertiary
    Behavior on textTertiary { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color warning: palette.warning
    Behavior on warning { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    // Elevation: shadow layers (light) and borders (dark).
    property color elevationCardBorder: palette.elevationCardBorder
    Behavior on elevationCardBorder { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color elevationCardShadow1: palette.elevationCardShadow1
    Behavior on elevationCardShadow1 { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    readonly property real elevationCardShadow1Y: palette.elevationCardShadow1Y * scale
    readonly property real elevationCardShadow1Blur: palette.elevationCardShadow1Blur * scale
    property color elevationCardShadow2: palette.elevationCardShadow2
    Behavior on elevationCardShadow2 { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    readonly property real elevationCardShadow2Y: palette.elevationCardShadow2Y * scale
    readonly property real elevationCardShadow2Blur: palette.elevationCardShadow2Blur * scale

    readonly property string fontUi: "Inter"
    readonly property string fontMono: "JetBrains Mono"
    readonly property string fontIndic: "Noto Sans Devanagari"
    // Type styles. Devanagari text falls back to fontIndic through fontconfig.
    readonly property font caption: Qt.font({ family: fontUi, pixelSize: Math.round(12 * scale), weight: 400 })
    readonly property real captionLineHeight: 1.45
    readonly property font small: Qt.font({ family: fontUi, pixelSize: Math.round(13 * scale), weight: 400 })
    readonly property real smallLineHeight: 1.45
    readonly property font body: Qt.font({ family: fontUi, pixelSize: Math.round(14 * scale), weight: 400 })
    readonly property real bodyLineHeight: 1.45
    readonly property font bodyLarge: Qt.font({ family: fontUi, pixelSize: Math.round(16 * scale), weight: 400 })
    readonly property real bodyLargeLineHeight: 1.45
    readonly property font title: Qt.font({ family: fontUi, pixelSize: Math.round(20 * scale), weight: 600 })
    readonly property real titleLineHeight: 1.25
    readonly property font headline: Qt.font({ family: fontUi, pixelSize: Math.round(24 * scale), weight: 600 })
    readonly property real headlineLineHeight: 1.25
    readonly property font display: Qt.font({ family: fontUi, pixelSize: Math.round(32 * scale), weight: 600 })
    readonly property real displayLineHeight: 1.25

    // Spacing on a 4 px grid: space8 is 8 px at scale 1.
    readonly property real space2: 2 * scale
    readonly property real space4: 4 * scale
    readonly property real space8: 8 * scale
    readonly property real space12: 12 * scale
    readonly property real space16: 16 * scale
    readonly property real space24: 24 * scale
    readonly property real space32: 32 * scale
    readonly property real space48: 48 * scale
    // Corner radii. Qt clamps a radius to half the height, so a pill's is large.
    readonly property real radiusControl: 6 * scale
    readonly property real radiusCard: 10 * scale
    readonly property real radiusPanel: 16 * scale
    readonly property real radiusPill: 9999
    readonly property real focusWidth: 2 * scale

    // Durations in milliseconds; 0 when movement is off.
    readonly property int motionMicro: animationsEnabled ? 120 : 0
    readonly property int motionStandard: animationsEnabled ? 200 : 0
    readonly property int motionPanel: animationsEnabled ? 320 : 0
    // For easing.type Easing.BezierSpline: two control points, then the end.
    readonly property var easingStandard: [0.2, 0, 0, 1, 1, 1]
    readonly property var easingEmphasised: [0.3, 0, 0, 1, 1, 1]
}
