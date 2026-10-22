package app.nuvora.tv

import android.app.Application
import android.content.Context
import app.nuvora.tv.data.SessionStore
import app.nuvora.tv.data.probeDeviceProfile
import coil3.ImageLoader
import coil3.PlatformContext
import coil3.SingletonImageLoader
import coil3.disk.DiskCache
import coil3.disk.directory
import coil3.memory.MemoryCache
import coil3.network.okhttp.OkHttpNetworkFetcherFactory
import okhttp3.OkHttpClient

class NuvoraApp : Application(), SingletonImageLoader.Factory {
    val store: SessionStore by lazy { SessionStore(this, probeDeviceProfile(this)) }

    /** Artwork needs the session too; images are immutable, so they cache on disk for good. */
    override fun newImageLoader(context: PlatformContext): ImageLoader {
        val withToken: OkHttpClient = store.http.newBuilder()
            .addInterceptor { chain ->
                val token = store.session.value?.token
                val request = chain.request()
                chain.proceed(if (token == null) request else request.newBuilder().header("Authorization", "Bearer $token").build())
            }
            .build()
        return ImageLoader.Builder(context)
            .components { add(OkHttpNetworkFetcherFactory(callFactory = { withToken })) }
            .memoryCache { MemoryCache.Builder().maxSizePercent(context, 0.2).build() }
            .diskCache { DiskCache.Builder().directory(context.cacheDir.resolve("artwork")).maxSizeBytes(256L * 1024 * 1024).build() }
            .build()
    }
}

val Context.store: SessionStore get() = (applicationContext as NuvoraApp).store
