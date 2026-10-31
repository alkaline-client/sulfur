plugins {
    alias(libs.plugins.kotlin.multiplatform) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.compose) apply false
}

// `./gradlew` with no task builds the one-file executable (see app/build.gradle.kts).
defaultTasks(":app:packageSingleExecutable")
