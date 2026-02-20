# Add project specific ProGuard rules here.
# UniFFI 使用 JNA，需要保留相关类
-keep class com.sun.jna.** { *; }
-keep class uniffi.** { *; }
-keep class com.bliailk.automotion.** { *; }
-dontwarn com.sun.jna.**
