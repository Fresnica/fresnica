# UniFFI's JNA interface method names map directly to exported Rust symbols.
# Keep the generated binding package stable when the host application enables R8.
-keep class com.fresnica.core.** { *; }

# React Native discovers module methods from the compiled native module surface.
-keep class com.fresnica.core.reactnative.** { *; }
