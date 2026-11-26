//! Pure `#![no_std]` Tier 1 violations for Minecraft Modding (Kilt / Fabric / Quilt / `NeoForge` / Mixin)
//! and Minecraft 1.21+ `.mcfunction` Data Packs (`GF080`).

use core::fmt;
use serde::{Deserialize, Serialize};

/// Native Minecraft JVM/Kotlin Mod & Mixin violations (`MCMOD-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MinecraftModViolation {
    /// `@Overwrite` Mixin destroys cross-mod compatibility; use `@Inject` or `MixinExtras` `@WrapOperation`.
    MixinOverwrite,
    /// Importing `net.minecraft.client.*` in common/server source crashes dedicated servers.
    ClientOnlyClassInCommon,
    /// Blocking `Thread.sleep(...)` on the Minecraft server/client tick thread freezes TPS/FPS.
    ServerThreadBlock,
    /// Mixin `@Inject` / `@Redirect` method missing unique `modid$` prefix or `cancellable` guard.
    UnsafeMixinTarget,
}

impl MinecraftModViolation {
    /// Canonical Minecraft mod rule code (`MCMOD-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 4] = [
            "MCMOD-MIXIN-OVERWRITE",
            "MCMOD-CLIENT-IN-COMMON",
            "MCMOD-SERVER-THREAD-BLOCK",
            "MCMOD-UNSAFE-MIXIN",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REMS: [&str; 4] = [
            "Replace `@Overwrite` with `@Inject` or MixinExtras `@WrapOperation` / `@ModifyExpressionValue` to preserve mod compatibility.",
            "Move `net.minecraft.client.*` references into client-only source sets (`src/client/`) or guard with side checks.",
            "Never call `Thread.sleep()` on the Minecraft tick thread; schedule work via server tick counters or async executors.",
            "Prefix Mixin helper methods with `<modid>$` and prefer MixinExtras `@WrapOperation` over brittle `@Redirect`.",
        ];
        REMS[self as usize]
    }
}

/// Native Minecraft 1.21+ `.mcfunction` Data Pack violations (`MCFUNC-*`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum McFunctionViolation {
    /// Leading `/` command prefix inside a `.mcfunction` file (invalid in Brigadier function files).
    LeadingSlashCommand,
    /// Malformed 1.20.2+/1.21+ `$` macro line (missing `$(var)` or `$(var)` on a non-`$` line).
    InvalidMacroSyntax,
    /// Unbounded `@e` selector without `type=`/`limit=`/`distance=` filter causing server TPS lag.
    UnboundedEntitySelector,
    /// Legacy 1.20 plural folder `data/<ns>/functions/` instead of 1.21+ singular `data/<ns>/function/`.
    LegacyPluralFolder121,
    /// Dangerous server-admin command (`stop`, `op`, `deop`, `ban`) embedded in a datapack function.
    UnsafeAdminCommand,
}

impl McFunctionViolation {
    /// Canonical `.mcfunction` rule code (`MCFUNC-*`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        const CODES: [&str; 5] = [
            "MCFUNC-LEADING-SLASH",
            "MCFUNC-MACRO-SYNTAX",
            "MCFUNC-UNBOUNDED-SELECTOR",
            "MCFUNC-121-PLURAL-FOLDER",
            "MCFUNC-UNSAFE-ADMIN-CMD",
        ];
        CODES[self as usize]
    }

    /// Actionable root-cause remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        const REMS: [&str; 5] = [
            "Remove the leading `/` from commands inside `.mcfunction` files.",
            "Prefix macro lines with `$` and include at least one `$(param)` placeholder (and never use `$(...)` on non-macro lines).",
            "Constrain `@e[...]` selectors with `type=`, `tag=`, `distance=`, or `limit=` to prevent TPS lag spikes.",
            "Rename `data/<namespace>/functions/` to singular `data/<namespace>/function/` for Minecraft 1.21+ (`pack_format >= 48`).",
            "Remove server-administration commands (`stop`, `op`, `deop`, `ban`) from datapack `.mcfunction` files.",
        ];
        REMS[self as usize]
    }
}

/// Unified closed sum type for Minecraft Modding & `.mcfunction` Data Packs (`GF080`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum MinecraftViolation {
    /// JVM / Kotlin Minecraft Mod & Mixin violation.
    Mod(MinecraftModViolation),
    /// Minecraft 1.21+ `.mcfunction` Data Pack violation.
    Function(McFunctionViolation),
}

impl MinecraftViolation {
    /// Canonical rule code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Mod(v) => v.code(),
            Self::Function(v) => v.code(),
        }
    }

    /// Actionable remediation.
    #[must_use]
    pub const fn remediation(self) -> &'static str {
        match self {
            Self::Mod(v) => v.remediation(),
            Self::Function(v) => v.remediation(),
        }
    }
}

impl fmt::Display for MinecraftViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
