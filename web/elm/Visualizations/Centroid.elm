module Centroid exposing (..)

import Array exposing (Array)
import Color exposing (Color)
import Path
import Shape exposing (Arc, defaultPieConfig)
import TypedSvg exposing (g, svg, text_)
import TypedSvg.Attributes exposing (fill, stroke, transform, viewBox)
import TypedSvg.Attributes.InPx exposing (r)
import TypedSvg.Core exposing (Svg)
import TypedSvg.Types exposing (Paint(..), Transform(..))
import TypedSvg.Attributes exposing (height)
import TypedSvg.Core exposing (text)
import TypedSvg.Types exposing (px)
import TypedSvg.Attributes exposing (fontFamily)
import TypedSvg.Attributes exposing (fontSize)
import TypedSvg.Attributes exposing (fontWeight)
import TypedSvg.Types exposing (FontWeight(..))
import TypedSvg.Attributes exposing (x)
import TypedSvg.Attributes exposing (y)

rgba255 : Int -> Int -> Int -> Float -> Color
rgba255 r g b a =
    Color.fromRgba { red = toFloat r / 255, green = toFloat g / 255, blue = toFloat b / 255, alpha = a }


colors : Array Color
colors =
    Array.fromList
        [ rgba255 31 119 180 0.5
        , rgba255 255 127 14 0.5
        , rgba255 44 159 44 0.5
        , rgba255 214 39 40 0.5
        , rgba255 148 103 189 0.5
        , rgba255 140 86 75 0.5
        , rgba255 227 119 194 0.5
        , rgba255 128 128 128 0.5
        , rgba255 188 189 34 0.5
        , rgba255 23 190 207 0.5
        ]


radius : Float -> Float -> Float
radius width height =
    min (width / 2) height / 2 - 10

{-
circular : List Arc -> Float -> Float -> Svg msg
circular arcs width height =
    let
        rad = radius width height
        makeSlice index datum =
            Path.element (Shape.arc datum)
                [ fill <| Paint <| Maybe.withDefault Color.black <| Array.get index colors
                , stroke <| Paint Color.black
                ]

        makeDot datum =
            let
                ( x, y ) =
                    Shape.centroid datum
            in
            circle [ cx x, cy y, r 5 ] []
    in
    g [ transform [ Translate rad rad ] ]
        [ g [] <| List.indexedMap makeSlice arcs
        , g [] <| List.map makeDot arcs
        ]
-}

annular : List Arc -> List String -> Float -> Svg msg
annular arcs labels rad =
    let
        zip = List.map2 Tuple.pair
        makeSlice index datum =
            Path.element (Shape.arc { datum | innerRadius = rad - 60 })
                [ fill <| Paint <| Maybe.withDefault Color.black <| Array.get index colors
                , stroke <| Paint Color.black
                ]

        makeLabel (datum, label) =
            let
                ( xloc, yloc ) =
                    Shape.centroid { datum | innerRadius = rad - 60 }
            in
            text_
                [ (x (px xloc))
                , (y (px yloc))
                , fontFamily [ "Helvetica", "sans-serif" ]
                , fontSize (px 10)
                , fontWeight FontWeightBold
                ]
                [ text label ]
    in
    g [ transform [ Translate (rad) (rad) ] ]
        [ g [] <| List.indexedMap makeSlice arcs
        , g [] <| List.map makeLabel (zip arcs labels)
        ]


view : List (String, Float) -> Float -> Float -> Int -> Int -> Svg msg
view data width height vbx vby =
    let
        sort = Basics.compare
        rad = radius width height
        sorted_data = data |> List.sortBy Tuple.second |> List.map Tuple.second
        sorted_labels = data |> List.sortBy Tuple.second |> List.map Tuple.first
        pieData =
            sorted_data |> Shape.pie { defaultPieConfig | outerRadius = rad, sortingFn = sort}
    in
    svg [
        viewBox 0 0 (Basics.toFloat vbx) (Basics.toFloat vby)
    ]
        [
            annular pieData sorted_labels rad
        ]
