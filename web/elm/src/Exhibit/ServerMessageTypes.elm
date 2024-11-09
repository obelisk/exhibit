module Exhibit.ServerMessageTypes exposing (..)

import Dict exposing (Dict)
import Json.Decode exposing (Decoder, andThen, fail, field, map2, string, succeed)
import Exhibit.IO exposing (Poll, pollDecoder, nestWebsocketMessageDecoder)


type SuccessType
    = VoteRecorded



-- Websocket Types and Subtypes


type ReceivedMessage
    = NewSlideMessage SlideSettings
    | InitialPresentationDataMessage InitialPresentationData
    | DisconnectMessage String
    | RatelimiterResponseMessage RatelimiterResponse
    | NewPollMessage Poll
    | Success SuccessType
    | Error String



-- Message from the server when a new slide is shown. This needs to mirror
-- the rust type variation OutgoingUserMessage::NewSlide


type alias SlideSettings =
    { message : String
    , emojis : List String
    }



type RatelimiterResponse
    = Allowed (Dict String String)
    | Blocked String


type alias InitialPresentationData =
    { title : String, settings : Maybe SlideSettings }



-- WebSocket Decoders


receivedWebsocketMessageDecoder : Decoder ReceivedMessage
receivedWebsocketMessageDecoder =
    Json.Decode.oneOf
        [ Json.Decode.map NewSlideMessage newSlideMessageDecoder
        , Json.Decode.map InitialPresentationDataMessage initialPresentationDataMessageDecoder
        , Json.Decode.map DisconnectMessage (simpleMessageDecoder "Disconnect")
        , Json.Decode.map RatelimiterResponseMessage ratelimiterResponseMessageDecoder
        , Json.Decode.map NewPollMessage newPollMessageDecoder
        , Json.Decode.map Success successMessageDecoder
        , Json.Decode.map Error (simpleMessageDecoder "Error")
        ]




successMessageDecoder : Decoder SuccessType
successMessageDecoder =
    field "Success" string
        |> andThen
            (\value ->
                case value of
                    "Vote recorded" ->
                        succeed VoteRecorded

                    _ ->
                        fail ("Unknown Success: " ++ value)
            )


newSlideMessageDecoder : Decoder SlideSettings
newSlideMessageDecoder =
    nestWebsocketMessageDecoder "NewSlide" slideSettingDecoder


slideSettingDecoder : Decoder SlideSettings
slideSettingDecoder =
    map2 SlideSettings
        (field "message" string)
        (field "emojis" (Json.Decode.list string))


initialPresentationDataMessageDecoder : Decoder InitialPresentationData
initialPresentationDataMessageDecoder =
    nestWebsocketMessageDecoder "InitialPresentationData"
        (map2 InitialPresentationData
            (field "title" string)
            (field "settings" (Json.Decode.maybe slideSettingDecoder))
        )


simpleMessageDecoder : String -> Decoder String
simpleMessageDecoder key =
    field key string


dictMessageDecoder : String -> Decoder (Dict String String)
dictMessageDecoder key =
    field key (Json.Decode.dict string)


ratelimiterResponseMessageDecoder : Decoder RatelimiterResponse
ratelimiterResponseMessageDecoder =
    nestWebsocketMessageDecoder "RatelimiterResponse"
        (Json.Decode.oneOf
            [ Json.Decode.map Allowed (dictMessageDecoder "Allowed")
            , Json.Decode.map Blocked (simpleMessageDecoder "Blocked")
            ]
        )


newPollMessageDecoder : Decoder Poll
newPollMessageDecoder =
    nestWebsocketMessageDecoder "NewPoll" pollDecoder