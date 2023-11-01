module Join exposing (..)

import Browser
import Html exposing (Html, button, div, input, label, text, ul)
import Html.Attributes exposing (class, for, id, type_)
import Html.Events exposing (on, onClick)
import Http exposing (..)
import Json.Decode exposing (Decoder, map, field, string)
import Html.Attributes exposing (value)
import Html.Events exposing (onInput)


main =
    Browser.element { init = init, update = update, subscriptions = subscriptions, view = view}

type State
  = Disconnected
  | Joining
  | Success JoinPresentationResponse


type alias Model =
  { registration_key : String
  , state : State
  }

init : () -> (Model, Cmd Msg)
init _ =
  ({registration_key = "", state = Disconnected}, Cmd.none)

type alias JoinPresentationResponse = { url : String }

type Msg
    = JoinPresentation
    | ChangeRegistrationKey String
    | GotWebsocketAddress (Result Http.Error JoinPresentationResponse)

update : Msg -> Model -> (Model, Cmd Msg)
update msg model =
    case msg of
        JoinPresentation ->
              (model, Http.post
                { url = "/join"
                , body = (Http.stringBody "application/text" model.registration_key)
                , expect = Http.expectJson GotWebsocketAddress joinPresentationResponseDecoder
                })
        GotWebsocketAddress response ->
            case response of
                Ok joinPresentationResponse ->
                    ({model | state = Success joinPresentationResponse}, Cmd.none)

                Err _ ->
                    (model, Cmd.none)
        ChangeRegistrationKey newRegistrationKey ->
          ({ model | registration_key = newRegistrationKey }, Cmd.none)

subscriptions : Model -> Sub Msg
subscriptions model =
  Sub.none

view : Model -> Html Msg
view model =
    div [ class "container" ]
        [ div [ class "title-group" ]
            [ div [ class "title", id "title" ] [ text "Join A Presentation" ]
            , ul [ id "ratelimit-info" ] []
            ]
        , label [ for "registration_key" ] [ text "Registration Key:" ]
        , input [ type_ "text", id "registration_key", value model.registration_key, onInput ChangeRegistrationKey ] []
        , button [ onClick JoinPresentation ] [ text "Join Presentation" ]
        , div [ id "poll-container" ]
            [ div [ id "poll-message" ] []
            , div [ id "poll-options" ] []
            ]
        , div [ id "slide-message" ] []
        , div [ id "reaction-help" ] [ text "Send a reaction below" ]
        , div [ id "reaction-container" ] []
        ]


joinPresentationResponseDecoder : Decoder JoinPresentationResponse
joinPresentationResponseDecoder =
  map JoinPresentationResponse
    (field "url" string)