# UniFFI Kotlin/JNA loads the packaged native library at runtime.
# Keep generated Fresnica SDK bindings when consumers enable shrinking.
-keep class com.fresnica.sdk.** { *; }
-dontwarn com.sun.jna.**
