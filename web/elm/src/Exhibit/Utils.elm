module Exhibit.Utils exposing (..)

getAtIndex : List a -> Int -> Maybe a
getAtIndex list index =
    list
        |> List.drop index
        |> List.head

popLast : List a -> Maybe (a, List a)
popLast list =
    case list of
        [] ->
            Nothing

        [x] ->
            Just (x, [])

        x :: xs ->
            case popLast xs of
                Nothing ->
                    Nothing

                Just (last, rest) ->
                    Just (last, x :: rest)
