import org.jetbrains.compose.desktop.application.dsl.TargetFormat
import java.net.InetSocketAddress
import java.net.ProxySelector
import java.net.URI
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.security.MessageDigest
import java.time.Duration
import java.util.Properties
import java.util.zip.Inflater

plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.compose)
}

// ---------------------------------------------------------------------------
// Rust core: build `sulfur-ffi`, generate Kotlin bindings with UniFFI and ship
// the native library as a JNA classpath resource (`<os>-<arch>/<lib>`).
//
// Cross builds: -Psulfur.rustTarget=<triple> -Psulfur.jnaPrefix=<os-arch>
// ---------------------------------------------------------------------------

val repoRoot: File = rootDir
val rustProfile = providers.gradleProperty("sulfur.rustProfile").getOrElse("release")
val rustTarget = providers.gradleProperty("sulfur.rustTarget").orNull

val hostOs = System.getProperty("os.name").lowercase()
val isWindows = hostOs.startsWith("windows")
val isMac = hostOs.startsWith("mac")
val jnaPrefix = providers.gradleProperty("sulfur.jnaPrefix").getOrElse(
    (if (isWindows) "win32" else if (isMac) "darwin" else "linux") + "-" +
        when (val arch = System.getProperty("os.arch").lowercase()) {
            "amd64", "x86_64" -> "x86-64"
            "aarch64", "arm64" -> "aarch64"
            else -> arch
        }
)
val nativeLibName = when {
    jnaPrefix.startsWith("win32") -> "sulfur_ffi.dll"
    jnaPrefix.startsWith("darwin") -> "libsulfur_ffi.dylib"
    else -> "libsulfur_ffi.so"
}
val cargoOutDir = repoRoot.resolve(
    listOfNotNull("target", rustTarget, if (rustProfile == "dev") "debug" else rustProfile).joinToString("/")
)
val nativeLib = cargoOutDir.resolve(nativeLibName)

// Per-checkout build settings in the untracked local.properties (key=value).
val localProperties = Properties().apply {
    repoRoot.resolve("local.properties").takeIf { it.isFile }?.reader()?.use { load(it) }
}

// Microsoft Entra application (client) ID for Microsoft sign-in, approved by
// Mojang for the Minecraft API. Not a secret, but tied to whoever registered
// it, so it is never committed and is passed at build time, first found wins:
// -Psulfur.msaClientId=<id>, SULFUR_MSA_CLIENT_ID, or sulfur.msaClientId in local.properties.
val msaClientId = providers.gradleProperty("sulfur.msaClientId")
    .orElse(providers.environmentVariable("SULFUR_MSA_CLIENT_ID"))
    .orElse(provider { localProperties.getProperty("sulfur.msaClientId") })
    .map { it.trim() }
    .getOrElse("")

val cargoBuild by tasks.registering(Exec::class) {
    group = "rust"
    description = "Builds the Rust core library (sulfur-ffi)."
    workingDir = repoRoot
    environment("SULFUR_MSA_CLIENT_ID", msaClientId)
    inputs.property("msaClientId", msaClientId)
    commandLine(
        listOf("cargo", "build", "-p", "sulfur-ffi", "--profile", rustProfile) +
            (rustTarget?.let { listOf("--target", it) } ?: emptyList())
    )
    inputs.files(
        fileTree(repoRoot.resolve("core")),
        repoRoot.resolve("Cargo.toml"),
        repoRoot.resolve("Cargo.lock"),
    )
    outputs.file(nativeLib)
}

val generateBindings by tasks.registering(Exec::class) {
    group = "rust"
    description = "Generates Kotlin bindings for the Rust core with UniFFI."
    dependsOn(cargoBuild)
    val outDir = layout.buildDirectory.dir("generated/uniffi")
    inputs.file(nativeLib)
    outputs.dir(outDir)
    workingDir = repoRoot
    doFirst { outDir.get().asFile.deleteRecursively() }
    commandLine(
        "cargo", "run", "--quiet", "-p", "uniffi-bindgen", "--profile", rustProfile, "--",
        "generate", "--library", nativeLib.absolutePath,
        "--language", "kotlin", "--no-format",
        "--out-dir", outDir.get().asFile.absolutePath,
    )
}

val bundleNativeLib by tasks.registering(Sync::class) {
    group = "rust"
    description = "Places the Rust core library where JNA finds it on the classpath."
    dependsOn(cargoBuild)
    from(nativeLib) { into(jnaPrefix) }
    into(layout.buildDirectory.dir("generated/native"))
}

// ---------------------------------------------------------------------------
// MiSans (Xiaomi) for Chinese and Japanese text. Its license allows embedding
// it in an app that credits it, but not handing out the font files on their
// own, so the repository does not contain it: this task takes the variable
// font out of Xiaomi's download (range requests, ~20 MB instead of the whole
// 228 MB archive) and the build embeds it. When it is not available the app
// falls back to the system's CJK fonts.
//
// Offline builds: -Psulfur.cjkFont=/path/to/MiSansVF.ttf, or =none to skip.
// ---------------------------------------------------------------------------

abstract class FetchZipEntry : DefaultTask() {
    @get:Input abstract val url: Property<String>
    @get:Input abstract val entry: Property<String>
    @get:Input abstract val sha256: Property<String>
    @get:Input @get:Optional abstract val override: Property<String>
    @get:Internal abstract val cacheDir: DirectoryProperty
    @get:OutputDirectory abstract val outputDir: DirectoryProperty

    @TaskAction
    fun fetch() {
        val name = entry.get().substringAfterLast('/')
        val out = outputDir.get().asFile.resolve("fonts").apply { deleteRecursively(); mkdirs() }
        val cached = cacheDir.get().asFile.resolve("${sha256.get().take(12)}-$name")
        val bytes = try {
            when (val local = override.orNull) {
                "none" -> return
                null -> if (cached.isFile) cached.readBytes() else download().also { verify(it) }
                else -> File(local).readBytes()
            }
        } catch (e: Exception) {
            logger.warn("w: $name not embedded, the app will use system fonts for CJK text: ${e.message}")
            return
        }
        if (!cached.isFile && override.orNull == null) {
            cached.parentFile.mkdirs()
            cached.writeBytes(bytes)
        }
        out.resolve(name).writeBytes(bytes)
    }

    private fun verify(bytes: ByteArray) {
        val actual = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
        check(actual == sha256.get()) { "SHA-256 is $actual, expected ${sha256.get()} (did Xiaomi update the font?)" }
    }

    /** Reads only the zip's directory and the one entry, never the whole archive. */
    private fun download(): ByteArray {
        val client = HttpClient.newBuilder()
            .proxy(proxy())
            .followRedirects(HttpClient.Redirect.NORMAL)
            .connectTimeout(Duration.ofSeconds(30))
            .build()
        fun range(from: Long, to: Long): ByteArray {
            val request = HttpRequest.newBuilder(URI(url.get()))
                .header("Range", "bytes=$from-$to")
                .header("User-Agent", "Sulfur-build")
                .timeout(Duration.ofMinutes(10))
                .build()
            val response = client.send(request, HttpResponse.BodyHandlers.ofByteArray())
            check(response.statusCode() == 206) { "HTTP ${response.statusCode()} for ${url.get()}" }
            return response.body()
        }
        fun ByteArray.u16(at: Int) = (this[at].toInt() and 0xff) or ((this[at + 1].toInt() and 0xff) shl 8)
        fun ByteArray.u32(at: Int) = u16(at).toLong() or (u16(at + 2).toLong() shl 16)

        // The end-of-central-directory record sits in the last 64 KiB.
        val probe = HttpRequest.newBuilder(URI(url.get())).header("Range", "bytes=0-0").build()
        val size = client.send(probe, HttpResponse.BodyHandlers.discarding())
            .headers().firstValue("Content-Range").orElseThrow().substringAfter('/').toLong()
        val tailStart = maxOf(0, size - 65_557)
        val tail = range(tailStart, size - 1)
        val eocd = (tail.size - 22 downTo 0).first { tail.u32(it) == 0x06054b50L }
        val directory = range(tail.u32(eocd + 16), tail.u32(eocd + 16) + tail.u32(eocd + 12) - 1)

        var at = 0
        while (at + 46 <= directory.size && directory.u32(at) == 0x02014b50L) {
            val method = directory.u16(at + 10)
            val compressed = directory.u32(at + 20)
            val length = directory.u32(at + 24)
            val nameLength = directory.u16(at + 28)
            val header = directory.u32(at + 42)
            val path = String(directory, at + 46, nameLength, Charsets.ISO_8859_1)
            if (path.endsWith(entry.get()) && "__MACOSX" !in path) {
                val local = range(header, header + 29)
                val data = header + 30 + local.u16(26) + local.u16(28)
                val raw = range(data, data + compressed - 1)
                if (method == 0) return raw
                val inflater = Inflater(true).apply { setInput(raw) }
                val result = ByteArray(length.toInt())
                var done = 0
                while (done < result.size && !inflater.finished()) done += inflater.inflate(result, done, result.size - done)
                inflater.end()
                return result
            }
            at += 46 + nameLength + directory.u16(at + 30) + directory.u16(at + 32)
        }
        error("${entry.get()} not found in ${url.get()}")
    }

    /** Java's proxy properties, else the HTTPS_PROXY variable most shells set. */
    private fun proxy(): ProxySelector {
        if (System.getProperty("https.proxyHost") != null) return ProxySelector.getDefault()
        val env = System.getenv("HTTPS_PROXY") ?: System.getenv("https_proxy") ?: return ProxySelector.getDefault()
        val uri = URI(if ("://" in env) env else "http://$env")
        return ProxySelector.of(InetSocketAddress(uri.host, if (uri.port > 0) uri.port else 80))
    }
}

val fetchCjkFont = tasks.register<FetchZipEntry>("fetchCjkFont") {
    group = "build"
    description = "Fetches the MiSans variable font for CJK text."
    url = "https://hyperos.mi.com/font-download/MiSans.zip"
    entry = "MiSansVF.ttf"
    sha256 = "0ddef90648998900175cfdca9a6f087a2544c182f130b0ad4f7e94a03a115e79"
    override = providers.gradleProperty("sulfur.cjkFont")
    cacheDir = layout.dir(provider { gradle.gradleUserHomeDir.resolve("caches/sulfur-fonts") })
    outputDir = layout.buildDirectory.dir("generated/fonts")
}

// ---------------------------------------------------------------------------
// JavaFX, only for its WebView: Cleanroom's download page is shown inside the
// app (see feature/download/CleanroomPage.kt). Its jars carry native code per
// platform; Gradle cannot evaluate the Maven profiles that pick it, so the
// modules are listed with this platform's classifier.
// ---------------------------------------------------------------------------

val javafxVersion = "21.0.13"
val javafxPlatform = (if (isWindows) "win" else if (isMac) "mac" else "linux") +
    (if (System.getProperty("os.arch").lowercase() in setOf("aarch64", "arm64")) "-aarch64" else "")

kotlin {
    jvm()
    jvmToolchain(21)

    sourceSets {
        commonMain.dependencies {
            implementation(libs.compose.runtime)
            implementation(libs.compose.foundation)
            implementation(libs.compose.ui)
            implementation(libs.compose.resources)
            implementation(libs.compose.material3)
            implementation(libs.material.kolor)
        }
        jvmMain {
            kotlin.srcDir(generateBindings)
            resources.srcDir(bundleNativeLib)
            resources.srcDir(fetchCjkFont)
            dependencies {
                implementation(compose.desktop.currentOs)
                implementation(libs.kotlinx.coroutines.swing)
                implementation(libs.jna)
                for (module in listOf("base", "graphics", "controls", "media", "web", "swing")) {
                    implementation("org.openjfx:javafx-$module:$javafxVersion:$javafxPlatform") { isTransitive = false }
                }
            }
        }
        all {
            languageSettings.optIn("androidx.compose.material3.ExperimentalMaterial3ExpressiveApi")
            languageSettings.optIn("androidx.compose.material3.ExperimentalMaterial3Api")
        }
    }
}

compose.resources {
    packageOfResClass = "dev.sulfur.resources"
}

compose.desktop {
    application {
        mainClass = "dev.sulfur.MainKt"

        nativeDistributions {
            targetFormats(TargetFormat.Dmg, TargetFormat.Msi, TargetFormat.Deb, TargetFormat.Rpm, TargetFormat.AppImage)
            packageName = "Sulfur"
            packageVersion = "0.1.0"
            description = "A Minecraft launcher for everyone"
            vendor = "Sulfur"
            // jdk.jsobject: the JavaFX WebView's JavaScript bridge.
            modules("jdk.unsupported", "java.management", "jdk.jsobject")
            windows {
                iconFile = project.file("icons/sulfur.ico")
                menu = true
                perUserInstall = true
                upgradeUuid = "1f0c6f1e-7a35-4f0e-9a51-5b0f4c1d9e21"
            }
            macOS {
                bundleID = "dev.sulfur.launcher"
            }
            linux {
                iconFile = project.file("icons/sulfur.png")
                packageName = "sulfur"
            }
        }

        buildTypes.release.proguard {
            // JNA and the generated bindings rely on reflection.
            isEnabled = false
        }
    }
}

// Renders pages to PNG off-screen:
// ./gradlew :app:screenshots [-Pout=dir] [-Pportable=dir] [-Pshots=launch-light,settings-dark]
tasks.register<JavaExec>("screenshots") {
    group = "verification"
    description = "Renders each page of the app to PNG files without a display."
    val main = kotlin.jvm().compilations["main"]
    dependsOn(main.compileAllTaskName, "jvmProcessResources")
    classpath = files(main.output.allOutputs, main.runtimeDependencyFiles)
    mainClass.set("dev.sulfur.tools.ScreenshotsKt")
    // A small heap and the serial collector: one off-screen frame at a time needs little.
    jvmArgs("-Xmx640m", "-XX:+UseSerialGC", "-XX:TieredStopAtLevel=1", "-Dskiko.renderApi=SOFTWARE")
    args(
        providers.gradleProperty("out").getOrElse(layout.buildDirectory.dir("screenshots").get().asFile.path),
        providers.gradleProperty("portable").getOrElse(layout.buildDirectory.dir("screenshot-data").get().asFile.path),
        providers.gradleProperty("shots").getOrElse(""),
    )
}
