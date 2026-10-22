# kotlinx.serialization, OkHttp, Media3 and Coil ship their own rules.
# Keep the API models' serializers by name for clarity in crash reports.
-keepattributes *Annotation*, InnerClasses
-keep,includedescriptorclasses class app.nuvora.tv.data.**$$serializer { *; }
-keepclassmembers class app.nuvora.tv.data.** { *** Companion; }
