allprojects {
    repositories {
        google()
        mavenCentral()
        maven(url = "https://jitpack.io")
    }
}

val newBuildDir: Directory =
    rootProject.layout.buildDirectory
        .dir("../../build")
        .get()
rootProject.layout.buildDirectory.value(newBuildDir)

subprojects {
    val newSubprojectBuildDir: Directory = newBuildDir.dir(project.name)
    project.layout.buildDirectory.value(newSubprojectBuildDir)
}
subprojects {
    project.evaluationDependsOn(":app")
}

// mobile_scanner 7.2.1 skips KGP on AGP 9, while this application remains in
// Flutter's legacy-KGP compatibility mode for other plugins.
subprojects {
    if (name == "mobile_scanner") {
        pluginManager.apply("org.jetbrains.kotlin.android")
    }
}

subprojects {
    configurations.configureEach {
        resolutionStrategy.force("com.github.Dimezis:BlurView:version-2.0.6")
    }
}

tasks.register<Delete>("clean") {
    delete(rootProject.layout.buildDirectory)
}
