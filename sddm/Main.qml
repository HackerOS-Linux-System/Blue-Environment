import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import QtGraphicalEffects 1.15
import SddmComponents 2.0

Rectangle {
    id: root
    width: Screen.width
    height: Screen.height
    color: "#0d1117"

    // ============================================================
    // BACKGROUND - wallpaper with blur
    // ============================================================
    Image {
        id: bgImage
        anchors.fill: parent
        source: config.background !== "" ? config.background : ""
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        visible: false
    }

    // Blur effect on wallpaper
    FastBlur {
        anchors.fill: bgImage
        source: bgImage
        radius: 60
        visible: bgImage.status === Image.Ready
    }

    // Dark overlay for readability
    Rectangle {
        anchors.fill: parent
        color: "#0d1117"
        opacity: bgImage.status === Image.Ready ? 0.55 : 1.0

        Behavior on opacity {
            NumberAnimation { duration: 800; easing.type: Easing.OutCubic }
        }
    }

    // ============================================================
    // STARFIELD (decorative background dots)
    // ============================================================
    Repeater {
        model: 80
        Rectangle {
            property real randX: Math.random()
            property real randY: Math.random()
            x: randX * root.width
            y: randY * root.height
            width: Math.random() * 2 + 1
            height: width
            radius: width / 2
            color: "white"
            opacity: Math.random() * 0.4 + 0.1

            SequentialAnimation on opacity {
                loops: Animation.Infinite
                NumberAnimation {
                    to: Math.random() * 0.3 + 0.05
                    duration: Math.random() * 3000 + 1000
                    easing.type: Easing.InOutSine
                }
                NumberAnimation {
                    to: Math.random() * 0.4 + 0.15
                    duration: Math.random() * 3000 + 1000
                    easing.type: Easing.InOutSine
                }
            }
        }
    }

    // ============================================================
    // NEBULA GLOW (top-left decorative)
    // ============================================================
    RadialGradient {
        width: 700
        height: 700
        x: -200
        y: -200
        gradient: Gradient {
            GradientStop { position: 0.0; color: "#3d8ef015" }
            GradientStop { position: 1.0; color: "transparent" }
        }
    }

    // ============================================================
    // CLOCK (top-center)
    // ============================================================
    Column {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: 60
        spacing: 4

        Text {
            id: clockTime
            anchors.horizontalCenter: parent.horizontalCenter
            font.pixelSize: 72
            font.weight: Font.Light
            color: "#e8f0fe"
            text: Qt.formatTime(new Date(), "hh:mm")

            Timer {
                interval: 1000
                running: true
                repeat: true
                onTriggered: clockTime.text = Qt.formatTime(new Date(), "hh:mm")
            }
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            font.pixelSize: 18
            font.weight: Font.Normal
            color: "#8899bb"
            text: Qt.formatDate(new Date(), "dddd, MMMM d, yyyy")
        }
    }

    // ============================================================
    // LOGIN CARD (center)
    // ============================================================
    Rectangle {
        id: loginCard
        width: 420
        height: loginColumn.height + 60
        anchors.centerIn: parent
        anchors.verticalCenterOffset: 30
        radius: 20
        color: "#0d1117e8"
        border.color: "#1e2d4a"
        border.width: 1

        // Top accent line
        Rectangle {
            width: parent.width
            height: 2
            radius: 1
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: "transparent" }
                GradientStop { position: 0.5; color: "#3d8ef0" }
                GradientStop { position: 1; color: "transparent" }
            }
        }

        // Backdrop blur would be applied by window compositor
        // The color above provides fallback

        ColumnLayout {
            id: loginColumn
            width: parent.width - 60
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: parent.top
            anchors.topMargin: 30
            spacing: 16

            // User avatar
            Rectangle {
                Layout.alignment: Qt.AlignHCenter
                width: 80
                height: 80
                radius: 40
                color: "#1a2540"
                border.color: "#3d8ef0"
                border.width: 2

                Image {
                    id: userAvatar
                    anchors.fill: parent
                    anchors.margins: 3
                    source: userModel.count > 0
                        ? "file://" + config.avatarPath + userModel.data(userModel.index(userList.currentIndex, 0), 0x101) + ".face.icon"
                        : ""
                    fillMode: Image.PreserveAspectCrop
                    layer.enabled: true
                    layer.effect: OpacityMask {
                        maskSource: Rectangle {
                            width: userAvatar.width
                            height: userAvatar.height
                            radius: userAvatar.width / 2
                        }
                    }
                }

                // Fallback person icon
                Text {
                    anchors.centerIn: parent
                    text: "👤"
                    font.pixelSize: 32
                    visible: userAvatar.status !== Image.Ready
                }
            }

            // Username display
            Text {
                Layout.alignment: Qt.AlignHCenter
                text: userList.currentItem
                    ? userList.model.data(userList.model.index(userList.currentIndex, 0), Qt.DisplayRole)
                    : "User"
                font.pixelSize: 20
                font.weight: Font.Medium
                color: "#e8f0fe"
            }

            // User selector (if multiple users)
            ComboBox {
                id: userList
                Layout.fillWidth: true
                visible: userModel.count > 1
                model: userModel
                textRole: "name"

                background: Rectangle {
                    color: "#1a2540"
                    radius: 8
                    border.color: userList.activeFocus ? "#3d8ef0" : "#1e2d4a"
                    border.width: 1
                }

                contentItem: Text {
                    leftPadding: 12
                    text: userList.displayText
                    color: "#e8f0fe"
                    font.pixelSize: 13
                    verticalAlignment: Text.AlignVCenter
                }
            }

            // Password field
            Rectangle {
                Layout.fillWidth: true
                height: 48
                radius: 10
                color: "#1a2540"
                border.color: passwordField.activeFocus ? "#3d8ef0" : "#1e2d4a"
                border.width: 1

                Behavior on border.color {
                    ColorAnimation { duration: 150 }
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 14
                    anchors.rightMargin: 14
                    spacing: 10

                    Text {
                        text: "🔒"
                        font.pixelSize: 14
                        color: "#8899bb"
                    }

                    TextField {
                        id: passwordField
                        Layout.fillWidth: true
                        echoMode: TextInput.Password
                        placeholderText: "Password"
                        color: "#e8f0fe"
                        background: null
                        font.pixelSize: 14

                        Keys.onReturnPressed: doLogin()
                        Keys.onEnterPressed: doLogin()
                    }
                }
            }

            // Login button
            Rectangle {
                Layout.fillWidth: true
                height: 48
                radius: 10
                color: loginMouseArea.containsMouse ? "#5aa3ff" : "#3d8ef0"

                Behavior on color { ColorAnimation { duration: 100 } }

                Text {
                    anchors.centerIn: parent
                    text: "Sign In"
                    color: "white"
                    font.pixelSize: 14
                    font.weight: Font.Medium
                }

                MouseArea {
                    id: loginMouseArea
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: doLogin()
                }
            }

            // Session selector
            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: 8

                Text {
                    text: "Session:"
                    color: "#8899bb"
                    font.pixelSize: 12
                }

                ComboBox {
                    id: sessionList
                    model: sessionModel
                    textRole: "name"
                    width: 200

                    background: Rectangle {
                        color: "#1a2540"
                        radius: 6
                        border.color: sessionList.activeFocus ? "#3d8ef0" : "#1e2d4a"
                        border.width: 1
                        height: 28
                    }

                    contentItem: Text {
                        leftPadding: 8
                        text: sessionList.displayText
                        color: "#8899bb"
                        font.pixelSize: 11
                        verticalAlignment: Text.AlignVCenter
                    }
                }
            }

            // Error message
            Text {
                id: errorMessage
                Layout.alignment: Qt.AlignHCenter
                text: ""
                color: "#ff4757"
                font.pixelSize: 12
                visible: text !== ""
            }

            Item { height: 4 }
        }
    }

    // ============================================================
    // BOTTOM BAR - power actions + keyboard layout
    // ============================================================
    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: 50
        color: "#0d1117c0"

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 24
            anchors.rightMargin: 24

            // Keyboard layout
            Text {
                text: "⌨ " + keyboard.layouts[keyboard.currentLayout].shortName
                color: "#8899bb"
                font.pixelSize: 12
            }

            Item { Layout.fillWidth: true }

            // Reboot button
            BlueActionButton {
                icon: "↺"
                label: "Reboot"
                onClicked: sddm.reboot()
            }

            // Shutdown button
            BlueActionButton {
                icon: "⏻"
                label: "Shut Down"
                onClicked: sddm.powerOff()
            }
        }
    }

    // ============================================================
    // Helper function
    // ============================================================
    function doLogin() {
        if (passwordField.text === "") {
            errorMessage.text = "Please enter your password"
            return
        }

        var username = userList.model.data(userList.model.index(userList.currentIndex, 0), Qt.DisplayRole)
        var session = sessionList.model.data(sessionList.model.index(sessionList.currentIndex, 0), Qt.UserRole + 1)

        sddm.login(username, passwordField.text, session)
    }

    // Handle login result
    Connections {
        target: sddm

        function onLoginSucceeded() {
            errorMessage.text = ""
        }

        function onLoginFailed() {
            errorMessage.text = "Incorrect password. Please try again."
            passwordField.text = ""
            passwordField.forceActiveFocus()
        }
    }

    // ============================================================
    // Power button component
    // ============================================================
    component BlueActionButton: Rectangle {
        property string icon: ""
        property string label: ""
        signal clicked()

        width: labelText.width + 48
        height: 34
        radius: 8
        color: ma.containsMouse ? "#1a2540" : "transparent"
        border.color: ma.containsMouse ? "#1e2d4a" : "transparent"
        border.width: 1

        Behavior on color { ColorAnimation { duration: 100 } }

        RowLayout {
            anchors.centerIn: parent
            spacing: 6

            Text {
                text: icon
                color: "#8899bb"
                font.pixelSize: 14
            }

            Text {
                id: labelText
                text: label
                color: "#8899bb"
                font.pixelSize: 12
            }
        }

        MouseArea {
            id: ma
            anchors.fill: parent
            hoverEnabled: true
            onClicked: parent.clicked()
        }
    }

    Component.onCompleted: {
        passwordField.forceActiveFocus()
    }
}
