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

    // What every theme's palette has.
    component Palette: QtObject {
        property color accent
        property color accentSubtle
        property color bgBase
        property color bgSurface1
        property color bgSurface2
        property color bgSurface3
        property color borderStrong
        property color borderSubtle
        property color danger
        property color focus
        property color info
        property color modeDirect
        property color modeDirectRaw
        property color modeLook
        property color modeSandbox
        property color textOnAccent
        property color success
        property color textPrimary
        property color textSecondary
        property color textTertiary
        property color warning
        property color elevationCardBorder
        property color elevationOverlayBorder
        property color elevationCardShadow1
        property real elevationCardShadow1Y
        property real elevationCardShadow1Blur
        property color elevationCardShadow2
        property real elevationCardShadow2Y
        property real elevationCardShadow2Blur
        property color elevationOverlayShadow1
        property real elevationOverlayShadow1Y
        property real elevationOverlayShadow1Blur
        property color elevationOverlayShadow2
        property real elevationOverlayShadow2Y
        property real elevationOverlayShadow2Blur
    }
    readonly property Palette dark: Palette {
        accent: "#F2B544"
        accentSubtle: "#3A2C10"
        bgBase: "#121214"
        bgSurface1: "#1A1A1F"
        bgSurface2: "#222229"
        bgSurface3: "#2B2B33"
        borderStrong: "#75757F"
        borderSubtle: "#34343D"
        danger: "#F5675C"
        focus: "#F2B544"
        info: "#4AA3F0"
        modeDirect: "#F08A24"
        modeDirectRaw: "#F5675C"
        modeLook: "#4AA3F0"
        modeSandbox: "#3FB97A"
        textOnAccent: "#1A1300"
        success: "#3FB97A"
        textPrimary: "#EDEDF0"
        textSecondary: "#A9A9B4"
        textTertiary: "#9A9AA5"
        warning: "#F08A24"
        elevationCardBorder: "#34343D"
        elevationOverlayBorder: "#75757F"
        elevationCardShadow1: "#00000000"
        elevationCardShadow1Y: 0
        elevationCardShadow1Blur: 0
        elevationCardShadow2: "#00000000"
        elevationCardShadow2Y: 0
        elevationCardShadow2Blur: 0
        elevationOverlayShadow1: "#8C000000"
        elevationOverlayShadow1Y: 12
        elevationOverlayShadow1Blur: 40
        elevationOverlayShadow2: "#00000000"
        elevationOverlayShadow2Y: 0
        elevationOverlayShadow2Blur: 0
    }
    readonly property Palette light: Palette {
        accent: "#915500"
        accentSubtle: "#FFF1D6"
        bgBase: "#FAFAF7"
        bgSurface1: "#FFFFFF"
        bgSurface2: "#F3F3EF"
        bgSurface3: "#EAEAE4"
        borderStrong: "#83837B"
        borderSubtle: "#DCDCD4"
        danger: "#C42B1C"
        focus: "#915500"
        info: "#1C66AA"
        modeDirect: "#A14B00"
        modeDirectRaw: "#C42B1C"
        modeLook: "#1C66AA"
        modeSandbox: "#1A7446"
        textOnAccent: "#FFFFFF"
        success: "#1A7446"
        textPrimary: "#18181B"
        textSecondary: "#55555E"
        textTertiary: "#64646D"
        warning: "#A14B00"
        elevationCardBorder: "#00000000"
        elevationOverlayBorder: "#DCDCD4"
        elevationCardShadow1: "#0F18181B"
        elevationCardShadow1Y: 1
        elevationCardShadow1Blur: 2
        elevationCardShadow2: "#0F18181B"
        elevationCardShadow2Y: 4
        elevationCardShadow2Blur: 12
        elevationOverlayShadow1: "#2918181B"
        elevationOverlayShadow1Y: 8
        elevationOverlayShadow1Blur: 32
        elevationOverlayShadow2: "#1418181B"
        elevationOverlayShadow2Y: 2
        elevationOverlayShadow2Blur: 6
    }
    readonly property Palette oled: Palette {
        accent: "#F2B544"
        accentSubtle: "#2A1F08"
        bgBase: "#000000"
        bgSurface1: "#0B0B0D"
        bgSurface2: "#151518"
        bgSurface3: "#1E1E23"
        borderStrong: "#6C6C78"
        borderSubtle: "#26262C"
        danger: "#F5675C"
        focus: "#F2B544"
        info: "#4AA3F0"
        modeDirect: "#F08A24"
        modeDirectRaw: "#F5675C"
        modeLook: "#4AA3F0"
        modeSandbox: "#3FB97A"
        textOnAccent: "#1A1300"
        success: "#3FB97A"
        textPrimary: "#EDEDF0"
        textSecondary: "#A9A9B4"
        textTertiary: "#9A9AA5"
        warning: "#F08A24"
        elevationCardBorder: "#26262C"
        elevationOverlayBorder: "#6C6C78"
        elevationCardShadow1: "#00000000"
        elevationCardShadow1Y: 0
        elevationCardShadow1Blur: 0
        elevationCardShadow2: "#00000000"
        elevationCardShadow2Y: 0
        elevationCardShadow2Blur: 0
        elevationOverlayShadow1: "#8C000000"
        elevationOverlayShadow1Y: 12
        elevationOverlayShadow1Blur: 40
        elevationOverlayShadow2: "#00000000"
        elevationOverlayShadow2Y: 0
        elevationOverlayShadow2Blur: 0
    }
    readonly property Palette palette: mode === "light" ? light : mode === "oled" ? oled : dark

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
    property color elevationOverlayBorder: palette.elevationOverlayBorder
    Behavior on elevationOverlayBorder { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    property color elevationCardShadow1: palette.elevationCardShadow1
    Behavior on elevationCardShadow1 { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    readonly property real elevationCardShadow1Y: palette.elevationCardShadow1Y * scale
    readonly property real elevationCardShadow1Blur: palette.elevationCardShadow1Blur * scale
    property color elevationCardShadow2: palette.elevationCardShadow2
    Behavior on elevationCardShadow2 { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    readonly property real elevationCardShadow2Y: palette.elevationCardShadow2Y * scale
    readonly property real elevationCardShadow2Blur: palette.elevationCardShadow2Blur * scale
    property color elevationOverlayShadow1: palette.elevationOverlayShadow1
    Behavior on elevationOverlayShadow1 { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    readonly property real elevationOverlayShadow1Y: palette.elevationOverlayShadow1Y * scale
    readonly property real elevationOverlayShadow1Blur: palette.elevationOverlayShadow1Blur * scale
    property color elevationOverlayShadow2: palette.elevationOverlayShadow2
    Behavior on elevationOverlayShadow2 { enabled: theme.animationsEnabled; ColorAnimation { duration: theme.motionStandard } }
    readonly property real elevationOverlayShadow2Y: palette.elevationOverlayShadow2Y * scale
    readonly property real elevationOverlayShadow2Blur: palette.elevationOverlayShadow2Blur * scale

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
