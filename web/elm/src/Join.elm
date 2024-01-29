port module Join exposing (..)

import Browser
import Html exposing (..)
import Html.Attributes exposing (class, id, type_, value, src)
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
import Html.Attributes exposing (for)



-- Ports
port socketConnect : String -> Cmd msg
port sendMessage : String -> Cmd msg
port messageReceived : (String -> msg) -> Sub msg
port socketDisconnected : (String -> msg) -> Sub msg
port refreshSocket : (String -> msg) -> Sub msg


main =
    Browser.element { init = init, update = update, subscriptions = subscriptions, view = view }



type alias InputView =
    { settings : SlideSettings
    , poll : Maybe Poll
    , pollState : PollState
    }

type PollState
    = VoteUnsubmitted
    | VoteAttempted
    | VoteConfirmed
    | VoteErrored

type State
    = Disconnected
    | Reconnecting
    | Joining
    | Viewing InputView


type alias Model =
    { registration_key : String
    , title : String
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
    ( { registration_key = key, title = "Please Join A Presentation", response = Nothing, state = Disconnected }, initialMsg )


type Msg
    -- Handle messages around connections and general
    -- house keeping
    = AuthenticateToPresentation
    | JoinPresentation String
    | ChangeRegistrationKey String
    | GotWebsocketAddress (Result Http.Error JoinPresentationResponse)
    | GotWebsocketAddressSilentUpdate (Result Http.Error JoinPresentationResponse)
    | ReceivedWebsocketMessage String
    | SocketDisconnected String
    | RefreshSocket String
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
            ( model
            , Http.post
                { url = "/join"
                , body = Http.stringBody "application/text" model.registration_key
                , expect = Http.expectJson GotWebsocketAddress joinPresentationResponseDecoder
                }
            )
            

        -- Handle the authentication response from the server with the WebSocket address
        GotWebsocketAddress response ->
            case response of
                -- We successfully authenticated to the presentation,
                -- open the websocket via the port
                Ok joinPresentationResponse ->
                    update (JoinPresentation joinPresentationResponse.url) { model | state = Joining }

                Err err ->
                    let de = Debug.log "GotWebsocketAddress Error" err in
                    ( { model | state = Disconnected}, Cmd.none )

        -- Handle the response from the REST API with our websocket address
        -- We need to send a message to the port even before the websocket is
        -- open to force Elm to create it.
        JoinPresentation url ->
            ( model, Cmd.batch [ socketConnect url, sendMessage "Hello" ] )

        -- On the websocket being disconnected, we need to update the UI
        -- to tell the user this so they can decide what they want to do.
        SocketDisconnected _ ->
            ( { model | state = Reconnecting, title = "Disconnected From Server" }, Cmd.none )
        
        -- Refresh socket has some duplicated logic from the above AuthenticateToPresentation and GotResponse
        -- These are split to achieve silent reconnect
        RefreshSocket _ ->
            let de = Debug.log "Attempting AuthenticateToPresentation" in
            ( model
            , Http.post
                { url = "/join"
                , body = Http.stringBody "application/text" model.registration_key
                , expect = Http.expectJson GotWebsocketAddressSilentUpdate joinPresentationResponseDecoder
                }
            )
        GotWebsocketAddressSilentUpdate response ->
            case response of
                Ok joinPresentationResponse ->
                    ( {model | state = Joining}, Cmd.batch [ socketConnect joinPresentationResponse.url, sendMessage "Hello" ] )
                Err err ->
                    (model, Cmd.none)

        -- Handle all message types from the websocket and route to the
        -- appropriate handler
        ReceivedWebsocketMessage message ->
            case Json.Decode.decodeString receivedWebsocketMessageDecoder message of
                Ok (InitialPresentationDataMessage initialPresentationData) ->
                    let
                        initialInputView = InputView (SlideSettings "" []) Nothing VoteUnsubmitted
                    in
                        update (InitialPresentationDataEvent initialPresentationData) {model | state = (Viewing initialInputView)}

                Ok (NewSlideMessage slideSettings) ->
                    update (NewSlideEvent slideSettings) model
                  
                Ok (DisconnectMessage m) ->
                    update (SocketDisconnected m) model
                
                Ok(RatelimiterResponseMessage m) ->
                    ({model | response = Just m}, Cmd.none)
                
                Ok(NewPollMessage m) ->
                    case model.state of
                        Viewing inputView -> ({model | state = (Viewing {inputView | poll = Just m, pollState = VoteUnsubmitted})}, Cmd.none)
                        _ -> (model, Cmd.none)

                Ok(Success success_type) ->
                  case success_type of
                      VoteRecorded -> case model.state of
                          -- Update poll state to vote confirmed
                          Viewing inputView -> ({model | state = (Viewing {inputView | pollState = VoteConfirmed})}, Cmd.none)
                          _ -> (model, Cmd.none)

                Ok(Error err) ->
                    case model.state of
                          -- Remove poll on error, likely only case is already voted (slides went backwards)
                          Viewing inputView -> ({model | state = (Viewing {inputView | poll = Nothing})}, Cmd.none)
                          _ -> (model, Cmd.none)

                Err err ->
                    let _ = Debug.log "ReceivedWebsocketMessage message (Err err)" (Json.Decode.errorToString err) in
                    ( model, Cmd.none )

        InitialPresentationDataEvent initialPresentationData ->
            case ( initialPresentationData.settings, { model | title = initialPresentationData.title } ) of
                ( Just settings, mdl ) ->
                    update (NewSlideEvent settings) mdl

                ( Nothing, mdl ) ->
                    ( mdl, Cmd.none )

        -- If we receive this message, the Websocket must be open and working
        -- so we switch to the viewing state
        NewSlideEvent slideSettings ->
            ( { model | state = Viewing (InputView slideSettings Nothing VoteUnsubmitted) }, Cmd.none )


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
            let
                updatedModelState =
                    case model.state of
                        Viewing inputView -> 
                            {model | state = Viewing {inputView | pollState = VoteAttempted}}
                        _ -> 
                            model
            in
                (updatedModelState, sendMessage (encodePollResponse poll))
            



subscriptions : Model -> Sub Msg
subscriptions _ =
    Sub.batch
        [ messageReceived ReceivedWebsocketMessage
        , socketDisconnected SocketDisconnected
        , refreshSocket RefreshSocket
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
                    viewDisconnectedState model
                    
                Joining -> 
                    -- Loading/connecting state
                    viewJoiningState model
                    
                Reconnecting -> 
                    -- On websocket disconnect, show reconnecting state 
                    viewReconnectingState model
                    
                Viewing inputView ->
                    div [] [
                        -- Render optional poll for this slide
                        viewPoll model inputView
                        
                        -- Render emoji reaction container 
                        , viewEmojiControls model inputView 
                    ]
            ]
    ]


viewDisconnectedState : Model -> Html Msg
viewDisconnectedState _ =
    div [ class "container" ] [
        div [ class "container-type-row"] [
            span [class "container-type-icon"] [ img [src "/static/icons/disconnected.png"] [] ] 
            , span [class "container-type-text"] [text "No presentation"]
        ]
        , div [ class "container-title-row"] [
            span [class "container-title-text"] [text "Unable to connect to presentation"]
        ]
        , div [ class "container-paragraph-row"] [
            span [class "container-paragraph-text"] [text "Presentation invite link is either invalid or expired. Please try requesting a new /exhibit link."]
        ]
    ]

viewJoiningState : Model -> Html Msg
viewJoiningState _ =
    div [ class "container" ] [
        div [ class "container-type-row"] [
            span [class "container-type-icon"] [ img [src "/static/icons/connecting.png"] []] 
            , span [class "container-type-text"] [text "Connecting"]
        ]
        , div [ class "container-title-row"] [
            span [class "container-title-text"] [text "Connecting to Presentation, please wait"]
        ]
    ]

viewReconnectingState : Model -> Html Msg
viewReconnectingState _ =
    div [ class "container" ] [
        div [ class "container-type-row"] [
            span [class "container-type-icon"] [ img [src "/static/icons/connecting.png"] []] 
            , span [class "container-type-text"] [text "Websocket Connection"]
        ]
        , div [ class "container-title-row"] [
            span [class "container-title-text"] [text "Attempting to Reconnect to Websocket"]
        ]
        , div [ class "container-paragraph-row"] [
            span [class "container-paragraph-text"] [text "Please wait... If the issue persists please try refreshing to page or requesting a new /exhibit link."]
        ]
    ]

viewPoll : Model -> InputView -> Html Msg
viewPoll model inputView =
    case inputView.poll of
        Just poll -> 
            if inputView.pollState == VoteConfirmed then 
                -- Vote submitted, poll completed, show confirmation view
                div [ class "container" ] [
                    div [ class "container-type-row"] [
                        span [class "container-type-icon"] [ img [src "/static/icons/voted.png"] [] ] 
                        , span [class "container-type-text"] [text "Vote Received"]
                    ]
                    , div [ class "container-title-row"] [
                        span [class "container-title-text"] [text "Thank you for voting!"]
                    ]
                ]
            else 
                -- Show poll form and options
                div [ class "container" ] [
                    div [ class "container-type-row"] [
                        span [class "container-type-icon"] [ img [src "/static/icons/poll.png"] [] ] 
                        , span [class "container-type-text"] [text "Poll"]
                    ]
                    , div [ class "container-title-row"] [
                        span [class "container-title-text"] [text poll.name]
                    ]
                    , case poll.vote_type of
                        SingleBinary _ ->
                            div [] 
                                [ div [ class "poll-type-text"] [text "Select a single option"]
                                , div [class "poll-options-group"] 
                                    (List.indexedMap (\index option -> 
                                        label [for <| "poll-option-" ++ (String.fromInt index)] [
                                            input [ type_ "radio", name "poll-options", id <| "poll-option-" ++ (String.fromInt index), onClick (ChangeSingleBinaryPollAnswer option)] []
                                            , div [ class "poll-item"] [text option]
                                        ]) poll.options)
                            ]

                        MultipleBinary _ ->
                            div []
                                [ div [ class "poll-type-text"] [text "Select all options that apply"]
                                ,  div [class "poll-options-group"] 
                                (List.indexedMap (\index option -> 
                                    label [for <| "poll-option-" ++ (String.fromInt index)] [
                                        input [ type_ "checkbox", name "poll-options", id <| "poll-option-" ++ (String.fromInt index), onCheck (ChangeMultipleBinaryPollAnswer option) ] []
                                        , div [ class "poll-item"] [text option] 
                                    ]) poll.options)
                            ]
                        , case inputView.pollState of 
                            VoteUnsubmitted -> 
                                div [class "poll-submit-button", onClick (SendPollAnswer poll) ] [text "Submit Vote"]
                            VoteAttempted -> 
                                div [class "poll-submit-button disabled" ] [text "Sending Vote..."]
                            VoteErrored -> 
                                div [class "poll-submit-button disabled" ] [text "Unable to vote"]
                            _ -> text ""
                            
                ]
        _ -> 
            div [] []


viewEmojiControls : Model -> InputView -> Html Msg
viewEmojiControls model inputView =
    div [ class "container" ] [
        div [ class "container-type-row"] [
            span [class "container-type-icon"] [img [src "/static/icons/podium.png"] []] 
            , span [class "container-type-text"] [text "Send an Emoji"]
        ]
        , div [ class "container-title-row"] [
            span [class "container-title-text"] [
                if inputView.settings.message == "" then
                    text "Tap an emoji below to send a live reaction"
                else   
                    text inputView.settings.message
            ]
        ]
        , div [ class "reaction-container" ]
            (
                if List.isEmpty inputView.settings.emojis then
                    [div [class "no-emojis-for-slide"] [text "No emojis available for current slide"]]
                else 
                    List.map (\emoji -> div [ class "reaction-button", onClick (SendEmoji emoji 1)] [ text emoji ]) inputView.settings.emojis
            )
        , div [class "rate-limiting-message"] [
            case model.response of
                Just (Blocked response) -> 
                    div [] [text response]
                Just (Allowed _) -> 
                    div [] [text "Emoji Sent!"]
                _ -> 
                    text ""
        ]
    ]