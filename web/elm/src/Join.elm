port module Join exposing (..)

import Browser
import Html exposing (..)
import Html.Attributes exposing (class, id, type_, value)
import Html.Events exposing (onClick, onInput)
import Http exposing (..)

import Exhibit.IO exposing (joinPresentationResponseDecoder, JoinPresentationResponse, Poll, VoteType(..))
import Exhibit.UserMessageTypes exposing (encodeEmojiReaction, encodePollResponse)
import Exhibit.ServerMessageTypes exposing (InitialPresentationData, SlideSettings, RatelimiterResponse, ReceivedMessage(..), receivedWebsocketMessageDecoder, SuccessType(..), RatelimiterResponse(..))
import Json.Decode
import Dict
import Html.Attributes exposing (name)
import Html.Events exposing (onCheck)
import Html exposing (br)
import Html.Attributes exposing (placeholder)
import Task



-- Ports
port socketConnect : String -> Cmd msg
port closeSocket : () -> Cmd msg
port sendMessage : String -> Cmd msg
port messageReceived : (String -> msg) -> Sub msg
port socketDisconnected : (String -> msg) -> Sub msg


main =
    Browser.element { init = init, update = update, subscriptions = subscriptions, view = view }



type alias InputView =
    { settings : SlideSettings
    , poll : Maybe Poll
    }


type State
    = Disconnected
    | Joining
    | Authenticated JoinPresentationResponse
    | Viewing InputView


type alias Model =
    { registration_key : String
    , title : String
    , error : Maybe String
    , response : Maybe RatelimiterResponse
    , state : State
    }


init : Maybe String -> ( Model, Cmd Msg )
init registration_key =
    let
        -- Receive parsed query params or cookie from JS port, attempt auto connect
        (key, initialMsg) =
            case registration_key of
                Just keyVal ->
                 (keyVal, Task.succeed AuthenticateToPresentation |> Task.perform identity)
                Nothing -> 
                    ("", Cmd.none)
    in
    ( { registration_key = key, title = "Please Join A Presentation", error = Nothing, response = Nothing, state = Disconnected }, initialMsg )


type Msg
    -- Handle messages around connections and general
    -- house keeping
    = AuthenticateToPresentation
    | JoinPresentation String
    | LeavePresentation
    | ChangeRegistrationKey String
    | GotWebsocketAddress (Result Http.Error JoinPresentationResponse)
    | ReceivedWebsocketMessage String
    | SocketDisconnected String
      -- Handle updating the model when new messages are successfully
      -- parsed
    | InitialPresentationDataEvent InitialPresentationData
    | NewSlideEvent SlideSettings
    -- Handle changing of user state like changing poll answers
    | ChangeSingleBinaryPollAnswer String
    | ChangeMultipleBinaryPollAnswer String Bool
    -- Handle messages around user actions like sending a reaction
    | SendEmoji String Int
    | SendPollAnswer Poll


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        -- Take in the user's registration key
        ChangeRegistrationKey newRegistrationKey ->
            ( { model | registration_key = newRegistrationKey }, Cmd.none )

        -- Authenticate to the presentation
        AuthenticateToPresentation ->
            if model.state == Disconnected then
                ( { model | state = Joining, error = Nothing}
                , Http.post
                    { url = "/join"
                    , body = Http.stringBody "application/text" model.registration_key
                    , expect = Http.expectJson GotWebsocketAddress joinPresentationResponseDecoder
                    }
                )
            else
                ( model, Cmd.none )

        LeavePresentation -> 
            ( { model | state = Disconnected, title = "Please Join A Presentation", error = Nothing, registration_key = ""}, closeSocket ())

        -- Handle the authentication response from the server with the WebSocket address
        GotWebsocketAddress response ->
            case response of
                -- We successfully authenticated to the presentation,
                -- open the websocket via the port
                Ok joinPresentationResponse ->
                    update (JoinPresentation joinPresentationResponse.url) { model | state = Authenticated joinPresentationResponse, error = Nothing }

                Err _ ->
                    ( { model | state = Disconnected, error = Just "Unable to connect to presentation"}, Cmd.none )

        -- Handle the response from the REST API with our websocket address
        -- We need to send a message to the port even before the websocket is
        -- open to force Elm to create it.
        JoinPresentation url ->
            ( model, Cmd.batch [ socketConnect url, sendMessage "Hello" ] )

        -- On the websocket being disconnected, we need to update the UI
        -- to tell the user this so they can decide what they want to do.
        SocketDisconnected _ ->
            ( { model | state = Disconnected, title = "Disconnected From Server", error = Just "Socket closed, you can try refreshng the page" }, Cmd.none )

        -- Handle all message types from the websocket and route to the
        -- appropriate handler
        ReceivedWebsocketMessage message ->
            case Json.Decode.decodeString receivedWebsocketMessageDecoder message of
                Ok (InitialPresentationDataMessage initialPresentationData) ->
                    update (InitialPresentationDataEvent initialPresentationData) model

                Ok (NewSlideMessage slideSettings) ->
                    update (NewSlideEvent slideSettings) model
                  
                Ok (DisconnectMessage m) ->
                    update (SocketDisconnected m) model
                
                Ok(RatelimiterResponseMessage m) ->
                    ({model | response = Just m}, Cmd.none)
                
                Ok(NewPollMessage m) ->
                    case model.state of
                        Viewing inputView -> ({model | state = (Viewing {inputView | poll = Just m})}, Cmd.none)
                        _ -> (model, Cmd.none)
                Ok(Success success_type) ->
                  case success_type of
                      VoteRecorded -> case model.state of
                          -- Close the poll view
                          Viewing inputView -> ({model | state = (Viewing {inputView | poll = Nothing})}, Cmd.none)
                          _ -> (model, Cmd.none)
                Ok(Error err) ->
                    ({model | error = Just err}, Cmd.none)
                Err err ->
                    ( { model | error = Just (Json.Decode.errorToString err) }, Cmd.none )

        InitialPresentationDataEvent initialPresentationData ->
            case ( initialPresentationData.settings, { model | title = initialPresentationData.title } ) of
                ( Just settings, mdl ) ->
                    update (NewSlideEvent settings) mdl

                ( Nothing, mdl ) ->
                    ( mdl, Cmd.none )

        -- If we receive this message, the Websocket must be open and working
        -- so we switch to the viewing state
        NewSlideEvent slideSettings ->
            case model.state of
                -- If we're already in the viewing state, don't erase the other
                -- state data like the poll
                Viewing inputView ->
                    ( { model | state = Viewing { inputView | settings = slideSettings } }, Cmd.none )

                _ ->
                    ( { model | state = Viewing (InputView slideSettings Nothing) }, Cmd.none )
        -- Handlers for changing user state like changing poll answers
        ChangeSingleBinaryPollAnswer answer ->
            case model.state of
                Viewing inputView -> case inputView.poll of
                    Just poll -> ({model | state = Viewing {inputView | poll = Just {poll | vote_type = SingleBinary answer}}}, Cmd.none)
                    Nothing -> (model, Cmd.none)
                _ -> (model, Cmd.none)

        ChangeMultipleBinaryPollAnswer option answer ->
            case model.state of
                Viewing inputView -> case inputView.poll of
                    Just poll -> case poll.vote_type of
                        MultipleBinary options -> ({model | state = Viewing {inputView | poll = Just {poll | vote_type = MultipleBinary (Dict.insert option answer options)}}}, Cmd.none)
                        _ -> (model, Cmd.none)
                    Nothing -> (model, Cmd.none)
                _ -> (model, Cmd.none)
        -- Handlers for user submission events like reactions and poll answers
        SendEmoji emoji size ->
            (model, sendMessage (encodeEmojiReaction emoji size))
        
        SendPollAnswer poll ->
            (model, sendMessage (encodePollResponse poll))
            



subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.batch
        [ messageReceived ReceivedWebsocketMessage
        , socketDisconnected SocketDisconnected
        ]


view : Model -> Html Msg
view model =
    div [] [
        -- Top title bar
        div [ class "title-group" ]
            [ div [ class "title-gradient-text-container" ] [
                div [ class "title" ] 
                    [ span [] [text model.title ] ]
                ]
            , div [ class "title-group-divider" ] []
            ]
        , div [ class "body-container" ] [
            case model.state of
                Disconnected -> 
                    -- Disconnected means that the magic link connection string didn't work, show a nice error message
                    div [ class "container" ] [
                        div [ class "container-type-row"] [
                            span [class "container-type-icon"] [text "x"] 
                            , span [class "container-type-text"] [text "Error"]
                        ]
                        , div [ class "container-title-row"] [
                            span [class "container-title-text"] [text "Unable to connect to presentation"]
                        ]
                        , div [ class "container-paragraph-row"] [
                            span [class "container-paragraph-text"] [text "Session is either invalid or expired. Please try requesting a new invitation link or clear your browser's cookies."]
                        ]
                    ]
                Joining -> 
                    -- Loading/connecting state
                    div [ class "container" ] [
                        div [ class "container-type-row"] [
                            span [class "container-type-icon"] [text "..."] 
                            , span [class "container-type-text"] [text "Connecting"]
                        ]
                        , div [ class "container-title-row"] [
                            span [class "container-title-text"] [text "Connecting to Presentation, please wait"]
                        ]
                    ]
                Viewing inputView ->
                    div [] [
                        -- If there is an active poll, render the poll container at the top
                        case inputView.poll of
                            Just poll -> 
                                div [ class "container" ] [
                                    div [ class "container-type-row"] [
                                        span [class "container-type-icon"] [text "(poll icon)"] 
                                        , span [class "container-type-text"] [text "Poll"]
                                    ]
                                    , div [ class "container-title-row"] [
                                        span [class "container-title-text"] [text poll.name]
                                    ]
                                    , div [ class "container-paragraph-row"] [
                                        case poll.vote_type of
                                            SingleBinary _ ->
                                                div [] 
                                                    (List.map (\option -> 
                                                        div [class "poll-option"] [
                                                            label [class "poll-option-label"] [ text option ]
                                                            , input [ type_ "radio", name "poll-options", onClick (ChangeSingleBinaryPollAnswer option)] []
                                                        ]) poll.options)
                                            MultipleBinary _ ->
                                                div [] 
                                                    (List.map (\option -> 
                                                        div [class "poll-option"] [
                                                            label [class "poll-option-label"] [ text option ]
                                                            , input [ type_ "checkbox", name "poll-options", onCheck (ChangeMultipleBinaryPollAnswer option) ] []
                                                        ]) poll.options)
                                            , input [type_ "button", name "poll-options-submit", onClick (SendPollAnswer poll) ] [text "Vote"]
                                    ]
                                ]
                            _ -> div [] []
                        -- Always show emoji reaction container 
                        , div [ class "container" ] [
                            div [ class "container-type-row"] [
                                span [class "container-type-icon"] [text "->"] 
                                , span [class "container-type-text"] [text "Live Interaction"]
                            ]
                            , div [ class "container-title-row"] [
                                span [class "container-title-text"] [text "Tap an emoji below to send a live reaction"]
                            ]
                            , div [ class "reaction-container" ]
                                (List.map (\emoji -> div [ class "reaction-button", onClick (SendEmoji emoji 1)] [ text emoji ]) inputView.settings.emojis)
                        ]
                    ]
                    
                _ -> 
                    div [] []
            ]
    ]


    -- case model.response of
    --             Just (Allowed responses) -> 
    --                 ul [ id "ratelimit-info" ] 
    --                     (List.map (\response -> li [class "ratelimiter-response"] [text ((Tuple.first response) ++ ": " ++ (Tuple.second response))] ) (Dict.toList responses))
    --             Just (Blocked response) -> 
    --                 div [class "warning"] [text ("Message was not sent: " ++ response)]
    --             Nothing -> 
    --                 div [] []