//! The Slint window of the example `shader-playground`.

slint::slint! {
    import { Slider, VerticalBox, HorizontalBox } from "std-widgets.slint";

    export component Playground inherits Window {
        title: "slang-slint shader playground";
        preferred-width: 640px;
        preferred-height: 480px;
        in property <image> view;
        in-out property <float> brightness <=> brightness-slider.value;
        in-out property <float> scale <=> scale-slider.value;
        out property <length> view-width: area.width;
        out property <length> view-height: area.height;

        VerticalBox {
            area := Rectangle {
                vertical-stretch: 1;
                Image {
                    width: parent.width;
                    height: parent.height;
                    source: root.view;
                    image-fit: fill;
                }
            }
            HorizontalBox {
                Text {
                    text: "Brightness";
                    vertical-alignment: center;
                    min-width: 80px;
                }
                brightness-slider := Slider {
                    minimum: 0;
                    maximum: 1;
                    value: 1;
                }
            }
            HorizontalBox {
                Text {
                    text: "Scale";
                    vertical-alignment: center;
                    min-width: 80px;
                }
                scale-slider := Slider {
                    minimum: 0;
                    maximum: 4;
                    value: 1;
                }
            }
        }
    }
}
