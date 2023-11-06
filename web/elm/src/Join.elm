port module Join exposing (..)

import Browser
import Html exposing (Html, button, div, input, label, text, ul, li)
import Html.Attributes exposing (class, for, id, type_, value)
import Html.Events exposing (onClick, onInput)
import Http exposing (..)

import UserMessageTypes exposing (..)
import ServerMessageTypes exposing (..)
import Json.Decode
import Dict
import Html.Attributes exposing (name)
import Html.Attributes exposing (selected)
import Html.Events exposing (onCheck)



-- Ports
port socketConnect : String -> Cmd msg
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


init : () -> ( Model, Cmd Msg )
init _ =
    ( { registration_key = "", title = "Please Join A Presentation", error = Nothing, response = Nothing, state = Disconnected }, Cmd.none )


type Msg
    -- Handle messages around connections and general
    -- house keeping
    = AuthenticateToPresentation
    | JoinPresentation String
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
                ( model
                , Http.post
                    { url = "/join"
                    , body = Http.stringBody "application/text" model.registration_key
                    , expect = Http.expectJson GotWebsocketAddress joinPresentationResponseDecoder
                    }
                )

            else
                ( model, Cmd.none )

        -- Handle the authentication response from the server with the WebSocket address
        GotWebsocketAddress response ->
            case response of
                -- We successfully authenticated to the presentation,
                -- open the websocket via the port
                Ok joinPresentationResponse ->
                    update (JoinPresentation joinPresentationResponse.url) { model | state = Authenticated joinPresentationResponse }

                Err _ ->
                    ( model, Cmd.none )

        -- Handle the response from the REST API with our websocket address
        -- We need to send a message to the port even before the websocket is
        -- open to force Elm to create it.
        JoinPresentation url ->
            ( model, Cmd.batch [ socketConnect url, sendMessage "Hello" ] )

        -- On the websocket being disconnected, we need to update the UI
        -- to tell the user this so they can decide what they want to do.
        SocketDisconnected _ ->
            ( { model | state = Disconnected, title = "Disconnected" }, Cmd.none )

        -- Handle all message types from the websocket and route to the
        -- appropriate handler
        ReceivedWebsocketMessage message ->
            case Json.Decode.decodeString receivedWebsocketMessageDecorder message of
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
    div [ class "container" ]
        [ div [ class "title-group" ]
            [ div [ class "title", id "title" ] [ text model.title ]
            , case model.error of
                Just err -> div [] [ text err ]
                Nothing -> div [] [] 
            , case model.response of
                    Just (Allowed responses) -> ul [ id "ratelimit-info" ] 
                        (List.map (\response -> li [class "ratelimiter-response"] [text ((Tuple.first response) ++ ": " ++ (Tuple.second response))] ) (Dict.toList responses))
                    
                    Just (Blocked response) -> div [class "warning"] [text ("Message was not sent: " ++ response)]
                    Nothing -> div [] []
            ]
        , label [ for "registration_key" ] [ text "Registration Key:" ]
        , input [ type_ "text", id "registration_key", value model.registration_key, onInput ChangeRegistrationKey ] []
        , button [ onClick AuthenticateToPresentation ] [ text "Join Presentation" ]
        , case model.state of
            Viewing inputView ->
                case inputView.poll of
                    Just poll -> div [ id "poll-container" ]
                        [ div [ id "poll-message" ] [ text poll.name ]
                        , case poll.vote_type of
                            SingleBinary _ ->
                                div [ id "pole-options" ] 
                                    (List.map (\option -> 
                                        div [class "poll-option"] [
                                            label [class "poll-option-label"] [ text option ]
                                            , input [ type_ "radio", name "poll-options", onClick (ChangeSingleBinaryPollAnswer option)] []
                                        ]) poll.options)
                            MultipleBinary _ ->
                                div [ id "pole-options" ] 
                                    (List.map (\option -> 
                                        div [class "poll-option"] [
                                            label [class "poll-option-label"] [ text option ]
                                            , input [ type_ "checkbox", name "poll-options", onCheck (ChangeMultipleBinaryPollAnswer option) ] []
                                        ]) poll.options)
                            {-SingleValue _ _ ->
                                div [ id "pole-options" ] 
                                        (List.map (\option -> 
                                            div [class "poll-option"] [
                                                label [class "poll-option-label"] [ text option ]
                                                , input [ type_ "slider", name "poll-options", selected False ] []
                                            ]) poll.options)
                            MultipleValue _ ->
                                div [ id "pole-options" ] 
                                    (List.map (\option -> 
                                        div [class "poll-option"] [
                                            label [class "poll-option-label"] [ text option ]
                                            , input [ type_ "slider", name "poll-options", selected False ] []
                                        ]) poll.options) -}
                        , input [type_ "button", name "poll-options-submit", onClick (SendPollAnswer poll) ] [text "Vote"]
                        ]
                    _ -> div [] []
            _ -> div [] []
        , case model.state of
            Viewing inputView ->
                div [ id "full-reactions-container" ]
                    [ div [ id "slide-message" ] []
                    , div [ id "reaction-help" ] [ text "Send a reaction below" ]
                    , div [ id "reaction-container" ]
                        (List.map (\emoji -> div [ class "reaction-button", onClick (SendEmoji emoji 1)] [ text emoji ]) inputView.settings.emojis)
                    ]

            _ ->
                div [ id "full-reactions-container" ] []
        ]