module UserMessageTypes exposing (..)

import ServerMessageTypes exposing (Poll, VoteType)

import Json.Encode
import ServerMessageTypes exposing (VoteType(..))
import Dict

-- Messages from the user are objects under the user key
-- to denote what objects they can be deserialized into on
-- the server size. Here we can easily wrap all the types

type alias UserMessage a =
    { user : a }


encodeUserMessage : (a -> Json.Encode.Value) -> a -> Json.Encode.Value
encodeUserMessage encoder message =
    Json.Encode.object
        [ ( "User", encoder message )
        ]

encodeEmojiReaction : String -> Int -> String
encodeEmojiReaction emoji size =
    Json.Encode.encode 0
        (encodeUserMessage Json.Encode.object
            [ ( "Emoji"
              , Json.Encode.object
                    [ ( "emoji", Json.Encode.string emoji )
                    , ( "size", Json.Encode.int size )
                    ]
              )
            ]
        )

encodePollResponse : Poll -> String
encodePollResponse poll =
    Json.Encode.encode 0
        (encodeUserMessage Json.Encode.object
            [ ( "Vote"
              , Json.Encode.object
                    [ ( "poll_name", Json.Encode.string poll.name )
                    , ( "vote_type", encodeVoteType poll.vote_type )
                    ]
              )
            ]
        )

encodeVoteType : VoteType -> Json.Encode.Value
encodeVoteType vote_type =
    case vote_type of 
        SingleBinary choice -> Json.Encode.object [ ("SingleBinary", Json.Encode.object [ ("choice", Json.Encode.string choice) ]) ]
        MultipleBinary choices -> Json.Encode.object [
            ("MultipleBinary", Json.Encode.object [
                ("choices", 
                Json.Encode.object (
                    (List.map (\(choice, picked) -> (choice, Json.Encode.bool picked)) (Dict.toList choices))))])]
        