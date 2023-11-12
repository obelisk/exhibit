module ServerMessageTypes exposing (..)

import Dict exposing (Dict)
import Json.Decode exposing (Decoder, andThen, fail, field, map, map2, map3, string, succeed)


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



-- Message from the server when a new slide is shown. This needs to mirror
-- the rust type variation OutgoingUserMessage::NewSlide


type alias SlideSettings =
    { message : String
    , emojis : List String
    }


type
    VoteType
    -- A single selected choice (radio buttons)
    = SingleBinary String
      -- Multiple selected choices (check boxes)
    | MultipleBinary (Dict String Bool)



-- Single choice with slider
--| SingleValue String Int
-- Multiple selected choices with sliders
--| MultipleValue (Dict String Int)


type alias Poll =
    { name : String
    , options : List String
    , vote_type : VoteType
    }


type RatelimiterResponse
    = Allowed (Dict String String)
    | Blocked String


type alias InitialPresentationData =
    { title : String, settings : Maybe SlideSettings }



-- WebSocket Decoders


receivedWebsocketMessageDecorder : Decoder ReceivedMessage
receivedWebsocketMessageDecorder =
    Json.Decode.oneOf
        [ Json.Decode.map NewSlideMessage newSlideMessageDecoder
        , Json.Decode.map InitialPresentationDataMessage initialPresentationDataMessageDecoder
        , Json.Decode.map DisconnectMessage (simpleMessageDecoder "Disconnect")
        , Json.Decode.map RatelimiterResponseMessage ratelimiterResponseMessageDecoder
        , Json.Decode.map NewPollMessage newPollMessageDecoder
        , Json.Decode.map Success successMessageDecoder
        ]


nestWebsocketMessageDecoder : String -> Decoder a -> Decoder a
nestWebsocketMessageDecoder nest decoder =
    field nest decoder


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
    nestWebsocketMessageDecoder "NewPoll"
        (map3 Poll
            (field "name" string)
            (field "options" (Json.Decode.list string))
            (field "vote_type" voteTypeDecoder)
        )


voteTypeDecoder : Decoder VoteType
voteTypeDecoder =
    Json.Decode.oneOf
        [ Json.Decode.map SingleBinary (field "SingleBinary" (field "choice" string))
        , Json.Decode.map MultipleBinary (field "MultipleBinary" (field "choices" (Json.Decode.dict Json.Decode.bool)))
        ]
