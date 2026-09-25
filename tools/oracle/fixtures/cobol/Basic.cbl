       IDENTIFICATION DIVISION.
       PROGRAM-ID. HELLO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-USER.
          05 WS-NAME PIC X(20).
          05 WS-AGE PIC 9(3).
       77 WS-TOTAL PIC 9(8).
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM INIT-PARA
           PERFORM PROCESS-PARA THRU PROCESS-EXIT
           CALL 'LOGGER'
           STOP RUN.
       INIT-PARA.
           MOVE 'guest' TO WS-NAME
           EXIT.
       PROCESS-PARA.
           ADD 1 TO WS-TOTAL
           GO TO PROCESS-EXIT.
       PROCESS-EXIT.
           EXIT.
