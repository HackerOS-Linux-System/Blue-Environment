# Blue Connect — release shrinking rules.
# The engine reaches TLS/keystore classes only through public JDK/Android APIs,
# nothing is looked up by reflection, so the defaults are enough. Keep line
# numbers so crash reports stay readable.
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile
