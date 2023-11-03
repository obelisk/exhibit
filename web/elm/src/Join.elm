port module Join exposing (..)

import Browser
import Html exposing (Html, button, div, input, label, text, ul)
import Html.Attributes exposing (class, for, id, type_)
import Html.Events exposing (onClick)
import Http exposing (..)
import Json.Decode exposing (Decoder, map, map2, field, string)
import Html.Attributes exposing (value)
import Html.Events exposing (onInput)
import Json.Decode exposing (oneOf)

-- Ports
port socketConnect : String -> Cmd msg
port sendMessage : String -> Cmd msg
port messageReceived : (String -> msg) -> Sub msg
port socketDisconnected : (String -> msg) -> Sub msg


main =
    Browser.element { init = init, update = update, subscriptions = subscriptions, view = view}

type State
  = Disconnected
  | Joining
  | Authenticated JoinPresentationResponse
  | Joined


type alias Model =
  { registration_key : String
  , state : State
  }

init : () -> (Model, Cmd Msg)
init _ =
  ({registration_key = "", state = Disconnected}, Cmd.none)

-- REST response from the server when we authenticate to the presentation
-- that tells us where our websocket is
type alias JoinPresentationResponse = { url : String }

type alias SlideSettings = {message : String, emojis : List String}
-- Message from the server when a new slide is shown. This needs to mirror
-- the rust type variation OutgoingUserMessage::NewSlide

type Msg
    = AuthenticateToPresentation
    | JoinPresentation String
    | ChangeRegistrationKey String
    | GotWebsocketAddress (Result Http.Error JoinPresentationResponse)
    | ReceivedWebsocketMessage String
    | SocketDisconnected String
  
type ReceivedMessage
    = NewSlide SlideSettings
    --| RatelimiterResponse
    --| InitialPresentationData
    --| Disconnect
    --| NewPoll

update : Msg -> Model -> (Model, Cmd Msg)
update msg model =
    case msg of
        -- Take in the user's registration key
        ChangeRegistrationKey newRegistrationKey ->
          ({ model | registration_key = newRegistrationKey }, Cmd.none)
        -- Authenticate to the presentation
        AuthenticateToPresentation ->
          if model.state == Disconnected then
              (model, Http.post
                { url = "/join"
                , body = (Http.stringBody "application/text" model.registration_key)
                , expect = Http.expectJson GotWebsocketAddress joinPresentationResponseDecoder
                })
            else
              (model, Cmd.none)
        -- Handle the authentication response from the server with the WebSocket address
        GotWebsocketAddress response ->
            case response of
                -- We successfully authenticated to the presentation,
                -- open the websocket via the port
                Ok joinPresentationResponse ->
                    update (JoinPresentation joinPresentationResponse.url) {model | state = Authenticated joinPresentationResponse}

                Err _ ->
                    (model, Cmd.none)

        JoinPresentation url ->
          (model, Cmd.batch [socketConnect url, sendMessage "Hello"])
        ReceivedWebsocketMessage message ->
          case Json.Decode.decodeString receivedWebsocketMessageDecorder message of
              Ok (NewSlide slideSettings) ->
                ({model | state = Joined}, Cmd.none)
              Err _ ->
                  (model, Cmd.none)
        SocketDisconnected _ ->
          ({model | state = Disconnected}, Cmd.none)


subscriptions : Model -> Sub Msg
subscriptions _ = Sub.batch [
    messageReceived ReceivedWebsocketMessage
  , socketDisconnected SocketDisconnected
  ]

view : Model -> Html Msg
view model =
    div [ class "container" ]
        [ div [ class "title-group" ]
            [ div [ class "title", id "title" ] [ text "Join A Presentation" ]
            , ul [ id "ratelimit-info" ] []
            ]
        , label [ for "registration_key" ] [ text "Registration Key:" ]
        , input [ type_ "text", id "registration_key", value model.registration_key, onInput ChangeRegistrationKey ] []
        , button [ onClick AuthenticateToPresentation ] [ text "Join Presentation" ]
        , div [ id "poll-container" ]
            [ div [ id "poll-message" ] []
            , div [ id "poll-options" ] []
            ]
        , div [ id "slide-message" ] []
        , div [ id "reaction-help" ] [ text "Send a reaction below" ]
        , div [ id "reaction-container" ] []
        ]

-- REST Decoders
joinPresentationResponseDecoder : Decoder JoinPresentationResponse
joinPresentationResponseDecoder =
  map JoinPresentationResponse
    (field "url" string)

-- WebSocket Decoders
receivedWebsocketMessageDecorder : Decoder ReceivedMessage
receivedWebsocketMessageDecorder =
    Json.Decode.oneOf
        [ Json.Decode.map NewSlide newSlideMessageDecoder
        ]


newSlideMessageDecoder : Decoder SlideSettings
newSlideMessageDecoder =
  map2 SlideSettings
    (field "message" string)
    (field "emojis" (Json.Decode.list string))