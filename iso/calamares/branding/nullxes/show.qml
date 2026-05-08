import QtQuick 2.5
import calamares.slideshow 1.0

Presentation {
    id: presentation
    Slide {
        Image { source: "logo.png"; anchors.centerIn: parent; opacity: 0.9 }
        Text  {
            anchors.bottom: parent.bottom
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottomMargin: 40
            text: "Installing NULLXES OS — minimal, fast, no gimmicks."
            color: "#E8E8E8"
            font.pixelSize: 18
        }
    }
    function onActivate() {}
    function onLeave() {}
}
