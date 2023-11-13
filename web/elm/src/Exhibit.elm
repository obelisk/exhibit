module Exhibit exposing (..)

import Http
import Json.Decode exposing (Decoder, field, map, string)
import Json.Decode exposing (map3)
import Dict exposing (Dict)
import Json.Encode



-- REST response from the server when we authenticate to the presentation
-- that tells us where our websocket is


type alias JoinPresentationResponse =
    { url : String }


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

pollDecoder : Decoder Poll 
pollDecoder =
    map3 Poll
        (field "name" string)
        (field "options" (Json.Decode.list string))
        (field "vote_type" voteTypeDecoder)


-- REST Decoders
joinPresentationResponseDecoder : Decoder JoinPresentationResponse
joinPresentationResponseDecoder =
    map JoinPresentationResponse
        (field "url" string)



{- This code is duplicated across Join and Present. It should be able to be abstracted
   I just can't figure it out yet.
-}
connectToPresentation : String -> (Result Http.Error a -> a) -> Decoder a -> Cmd a
connectToPresentation registration_key msg decoder =
    Http.post
        { url = "/join"
        , body = Http.stringBody "application/text" registration_key
        , expect = Http.expectJson msg decoder
        }


nestWebsocketMessageDecoder : String -> Decoder a -> Decoder a
nestWebsocketMessageDecoder nest decoder =
    field nest decoder

encodeMessageUnderKey : String -> (a -> Json.Encode.Value) -> a -> Json.Encode.Value
encodeMessageUnderKey key encoder message =
    Json.Encode.object
        [ ( key, encoder message )
        ]

encodePresenterMessage : (a -> Json.Encode.Value) -> a -> Json.Encode.Value
encodePresenterMessage encoder message =
    encodeMessageUnderKey  "Presenter" encoder message


voteTypeDecoder : Decoder VoteType
voteTypeDecoder =
    Json.Decode.oneOf
        [ Json.Decode.map SingleBinary (field "SingleBinary" (field "choice" string))
        , Json.Decode.map MultipleBinary (field "MultipleBinary" (field "choices" (Json.Decode.dict Json.Decode.bool)))
        ]

voteTypeEncoder : VoteType -> Json.Encode.Value
voteTypeEncoder vote_type =
    case vote_type of
        SingleBinary choice ->
            Json.Encode.object [
                ("SingleBinary ", Json.Encode.string choice)
            ]
        MultipleBinary choices ->
            Json.Encode.object [
                ("MultipleBinary", Json.Encode.dict identity Json.Encode.bool choices)
            ]

encodePollAsRequestTotalsMessage : Poll -> String
encodePollAsRequestTotalsMessage poll =
    Json.Encode.encode 0 (encodePresenterMessage (encodeMessageUnderKey "GetPollTotals" Json.Encode.object)
        [
            ("name", Json.Encode.string poll.name)
        ]
    )